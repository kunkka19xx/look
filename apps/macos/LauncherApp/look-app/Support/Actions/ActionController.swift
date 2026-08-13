import AppKit
import Combine
import Foundation

/// Runtime state for the Act pillar: holds the pending action awaiting confirm,
/// runs it, and keeps the last receipt for undo. Both producers (the `>` parser
/// and, later, the model planner) call `propose`.
/// One line of the AI session panel: a completed action ("Added \"Lunch\""), a
/// user question, or a (streaming) answer.
struct ActionSessionItem: Identifiable, Equatable {
    enum Kind: String { case action, user, answer }
    let id = UUID()
    var kind: Kind = .action { didSet { recomputeSegments() } }
    var text: String { didSet { recomputeSegments() } }
    /// Who produced an answer (model tag, "Apple Intelligence", "Calendar"),
    /// captured at creation so the chat header stays truthful per message.
    var source: String? = nil
    /// True while tokens are still arriving. Markdown is NOT parsed then: the
    /// parse is an FFI call plus a full-text scan, and at ~12 polls/sec over a
    /// growing answer that is quadratic work on the main thread. The view
    /// renders plain text during the stream and switches to parsed segments
    /// when this flips false (also avoids styling half-written code fences).
    var isStreaming: Bool = false { didSet { recomputeSegments() } }
    /// Parsed markdown, computed once the answer settles so the view renders
    /// without re-parsing on every frame. Empty for non-answers and mid-stream.
    private(set) var segments: [EngineBridge.AIMarkdownSegment] = []
    /// Files `@`-attached to this turn, kept on the item so the transcript
    /// still shows what a question was asked ABOUT after the input is cleared.
    var attachedPaths: [String] = []

    init(
        kind: Kind = .action,
        text: String,
        source: String? = nil,
        isStreaming: Bool = false,
        attachedPaths: [String] = []
    ) {
        self.kind = kind
        self.text = text
        self.source = source
        self.isStreaming = isStreaming
        self.attachedPaths = attachedPaths
        recomputeSegments()
    }

    private mutating func recomputeSegments() {
        segments = (kind == .answer && !isStreaming)
            ? EngineBridge.shared.aiMarkdownSegments(text)
            : []
    }
}

@MainActor
final class ActionController: ObservableObject {
    static let shared = ActionController()

    /// A disambiguation in progress: the tool call awaiting the user's pick.
    struct PendingChoice {
        let toolID: String
        var params: [String: String]
        let candidates: [ActionCandidate]
    }

    /// The file paths picked in the launcher, pushed by `LauncherView` as the
    /// picks change (this is a singleton with no view context). Picks survive
    /// the query changing to `>summarize`, which the row selection does not, so
    /// they are what a text op can actually aim at.
    var pickedFilePaths: [String] = []

    /// Files `@`-mentioned in the message being submitted. Pushed by
    /// `LauncherView` on submit and cleared with the input, so they belong to
    /// one turn rather than lingering like picks.
    var attachments = MentionAttachments()

    private func textOpSource() -> TextOpSource {
        TextOpSource.resolve(mentioned: attachments.paths, pickedFilePaths: pickedFilePaths)
    }

    @Published private(set) var pending: PlannedAction?
    @Published private(set) var pendingChoice: PendingChoice?
    @Published private(set) var lastReceipt: ActionReceipt?
    @Published private(set) var feedback: String = ""
    /// The action item the current `lastReceipt` can undo (answers may stack
    /// after it, so "last item" is not reliable).
    @Published private(set) var undoableItemID: UUID?
    /// True while the model is turning a `>` query into an action, so the UI can
    /// show a "thinking" indicator during the generation.
    @Published private(set) var isPlanning: Bool = false
    /// A model-interpreted file-recall request. The shell owns file results
    /// (open/reveal/quicklook live in the main panel), so it consumes this and
    /// runs the structured search there.
    struct RecallRequest: Equatable {
        let query: String
        let params: [String: String]
    }
    @Published private(set) var recallRequest: RecallRequest?
    func clearRecallRequest() { recallRequest = nil }

    /// A leftover disambiguation must not survive into an unrelated turn.
    func clearPendingChoice() { pendingChoice = nil }

    /// A listing the user just saw becomes the referent set: "remove this
    /// event" right after "what's on this week?" targets what was listed.
    func rememberListed(_ listing: ScheduleContextProvider.Listing) {
        if !listing.events.isEmpty {
            recentTargets = listing.events.map {
                RecentTarget(
                    domain: "calendar", id: $0.id,
                    label: "\($0.title)  ·  \(DatePhrase.format($0.start))")
            }
        } else if !listing.reminders.isEmpty {
            recentTargets = listing.reminders.map {
                RecentTarget(domain: "reminder", id: $0.id, label: $0.title)
            }
        }
    }

    /// The conversation half lives in its own controller; this one keeps the
    /// propose/confirm/undo state machine.
    let chat = ChatSessionController.shared

    private let planner: ActionPlanner
    private var planTask: Task<Void, Never>?
    private var idleTask: Task<Void, Never>?
    private var chatTask: Task<Void, Never>?
    private var lastWarm = Date.distantPast
    /// Bumped whenever ownership of the planning state changes (new keystroke,
    /// submit, cancel). A plan task compares its captured generation after the
    /// await, so a superseded task can never clobber `isPlanning` or propose a
    /// stale action - regardless of when its network call actually returns.
    private var planGeneration = 0

    /// How long the user must stop typing before the model runs. Short, because
    /// an in-flight call is cancelled cleanly on the next keystroke (Task cancel
    /// closes the connection; Ollama aborts generation on disconnect). The
    /// working indicator shows instantly regardless.
    private static let idleDelay: UInt64 = 300_000_000

    private init() {
        self.planner = ActionPlanner()
    }

    var isPresenting: Bool { pending != nil }

    /// A post-action result (success summary + undo, or an error message) is
    /// showing. Dismissed when the user types something new or hides the window.
    var hasResult: Bool { lastReceipt != nil || !feedback.isEmpty }

    var sessionActive: Bool { chat.isActive }

    func dismissResult() {
        lastReceipt = nil
        feedback = ""
    }

    /// Dismiss a transient result line (e.g. "Remembered …") on the next input.
    func clearFeedback() {
        if !feedback.isEmpty { feedback = "" }
    }

    /// Live preview while typing a `>` query. Two stages: an instant deterministic
    /// preview (the verb picks the action, the parser fills the spec) that tracks
    /// each keystroke with no model, plus a background model refinement that fires
    /// only once the user stops typing (idle) and upgrades the preview in place.
    func previewExplicitAIQuery(_ rawQuery: String) {
        let query = stripPrefix(rawQuery)

        guard !query.isEmpty else {
            handleComposeCleared()
            return
        }

        // Composing new input: supersede any in-flight preview.
        idleTask?.cancel()
        planTask?.cancel()
        planGeneration += 1

        // Warm the reminder cache while the user composes, so reminder mutations
        // ("complete milk") resolve against a fresh list by the time they Enter.
        // Throttled inside the service.
        EventKitService.shared.refreshReminderCache()
        EventKitService.shared.refreshEventCache()

        // Explicit `@` form: instant, exact, no model. Without a planner the
        // parser runs lenient (whole-phrase date resolution), so `@` entries
        // stay fully functional on any provider.
        let modelAvailable = planner.isAvailable
        if let raw = EngineBridge.shared.aiParseExplicit(">" + query, modelAvailable: modelAvailable),
           case .planned(let plan) = resolveOutcome(toolID: raw.toolID, params: raw.params) {
            isPlanning = false
            pending = plannedAction(toolID: raw.toolID, plan: plan)
            feedback = ""
            return
        }

        // Natural language: never show a rough/wrong title. Instead show the
        // working indicator IMMEDIATELY (no dead air) and normalize in the
        // background; the clean preview replaces the indicator when ready.
        guard modelAvailable else {
            // No planner: stay quiet while composing; Enter routes to the
            // provider's chat/answer path, and the footer teaches the `@` form.
            // Prewarm kicks the on-device model awake while the user types
            // (FoundationModels can report unavailable until first touched).
            AIQueryRouter.shared.prewarm(ThemeStore.shared.settings.aiProvider)
            pending = nil
            isPlanning = false
            feedback = ""
            return
        }
        pending = nil
        isPlanning = true
        // Warm the model + prompt cache now, while the user is still typing, so
        // the first real plan skips model load and prompt processing. Throttled.
        if Date().timeIntervalSince(lastWarm) > 90 {
            lastWarm = Date()
            Task { [planner] in await planner.warmUp() }
        }
        let generation = planGeneration
        idleTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.idleDelay)
            guard let self, !Task.isCancelled else { return }
            let call = await self.planner.plan(query: query)
            guard generation == self.planGeneration else { return }  // superseded
            self.isPlanning = false
            if let call, !Self.isUtilityCall(call) {
                // Live preview never pops a choice list mid-typing; Enter does.
                // Utility calls (recall/textop) only ever run on Enter.
                self.propose(call, allowChoice: false)
            }
            // Not an action: stay quiet. Enter routes the text to chat instead.
        }
    }

    /// Enter with no preview yet: force an immediate plan (deterministic first,
    /// then model). When a preview is already showing, handleSubmit confirms it
    /// instead of calling this.
    func submitExplicitAIQuery(_ rawQuery: String) {
        // A submit owns the planning state from here: supersede any in-flight
        // preview so its late return can't touch what this submit sets up.
        idleTask?.cancel()
        planTask?.cancel()
        planGeneration += 1
        let query = stripPrefix(rawQuery)
        guard !query.isEmpty else { return }

        EventKitService.shared.refreshReminderCache()
        EventKitService.shared.refreshEventCache()

        // The dispatch ladder lives in the Rust core (core/ai/src/route.rs),
        // shared with every shell: memory -> textop -> files -> explicit ->
        // plan -> chat. The memory tier has already executed when it answers.
        let modelAvailable = planner.isAvailable
        let route = EngineBridge.shared.aiRoute(
            input: query,
            memoryPath: MemoryStore.filePath ?? "",
            modelAvailable: modelAvailable)
        switch route {
        case .memory(let memoryFeedback):
            isPlanning = false
            pending = nil
            feedback = memoryFeedback
        case .textOp(let label, let instruction):
            isPlanning = false
            pending = nil
            if let note = chat.runTextOp(
                label: label, instruction: instruction, source: textOpSource()) {
                feedback = note
            }
        case .files:
            isPlanning = false
            pending = nil
            // An attached file settles it: "summary this file please?" contains
            // the word "file", so the deterministic recall parser claims it and
            // the shell would leave AI mode to show a file LIST - throwing away
            // the question. With something attached, the user is asking ABOUT
            // it, not searching for it. The parser cannot know that; only this
            // layer holds the attachments.
            if attachments.isEmpty {
                // Empty params = "parse the raw query"; the shell owns results.
                recallRequest = RecallRequest(query: query, params: [:])
            } else {
                chat.askChat(query, attachments: attachments)
            }
        case .explicit(let toolID, let params):
            if case .planned(let plan) = resolveOutcome(toolID: toolID, params: params) {
                isPlanning = false
                pending = plannedAction(toolID: toolID, plan: plan)
                feedback = ""
            } else if modelAvailable {
                // Deterministic parse resolved to nothing usable: let the
                // model read the same text before giving up to chat.
                startPlanTask(query)
            } else {
                chat.askChat(query, attachments: attachments)
            }
        case .plan:
            startPlanTask(query)
        case .chat:
            // No planner: hand the text to the provider's answer path instead
            // of dead-ending (askChat itself reports if no provider is usable).
            chat.askChat(query, attachments: attachments)
        }
    }

    /// The model turn of the ladder. The message joins the transcript NOW; the
    /// planner round-trip ("is this an action?") happens under the Thinking
    /// indicator, not before the user sees their own message.
    private func startPlanTask(_ query: String) {
        isPlanning = true
        chat.append(ActionSessionItem(kind: .user, text: query, attachedPaths: attachments.paths))
        chat.saveConversation()
        let generation = planGeneration
        planTask = Task { [weak self] in
            guard let self else { return }
            let call = await self.planner.plan(query: query)
            guard generation == self.planGeneration else { return }  // superseded
            self.isPlanning = false
            if let call {
                self.handlePlannedCall(call, rawQuery: query)
            } else {
                // Not an add-action: treat it as a chat turn in the session.
                self.chat.askChat(query, userItemAppended: true, attachments: self.attachments)
            }
        }
    }

    private static func isUtilityCall(_ call: ToolCall) -> Bool {
        call.toolID == "files.recall" || call.toolID == "clipboard.textop"
    }

    /// Routes a planner call: utility intents execute directly (file recall in
    /// the main panel, clipboard op in the session); calendar/reminder actions
    /// go through the propose/confirm flow.
    private func handlePlannedCall(_ call: ToolCall, rawQuery: String) {
        switch call.toolID {
        case "files.recall":
            // Same rule as the deterministic files route: with a file attached,
            // the question is about that file, not a search for others.
            if attachments.isEmpty {
                recallRequest = RecallRequest(query: rawQuery, params: call.params)
            } else {
                // `startPlanTask` already put the message in the transcript, so
                // this must NOT add it again.
                chat.askChat(rawQuery, userItemAppended: true, attachments: attachments)
            }
        case "clipboard.textop":
            guard let instruction = call.params["instruction"], !instruction.isEmpty else {
                chat.askChat(rawQuery, userItemAppended: true, attachments: attachments)
                return
            }
            let label = String(instruction.prefix(48))
            // Same here: the raw message is already the turn, so the op adds
            // its answer to it rather than a second, differently-worded turn.
            if let note = chat.runTextOp(
                label: label, instruction: instruction, source: textOpSource(),
                userItemAppended: true) {
                feedback = note
            }
        default:
            propose(call)
        }
    }

    /// A chat turn in the session: the question and a streaming answer stack as
    /// items, with the session (including performed actions) as conversation
    /// context. This is what a non-action `>` query becomes on Enter.
    /// `userItemAppended` = the submit already put the message in the transcript
    /// (it shows instantly while the planner thinks); don't add it twice.
    private func stripPrefix(_ raw: String) -> String {
        let text = raw.hasPrefix(">") ? String(raw.dropFirst()) : raw
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var awaitingChoice: Bool { pendingChoice != nil }

    /// Destructive tools that mean the same intent in the other domain, so
    /// "remove it" right after adding a REMINDER remaps event-cancel to
    /// reminder-remove instead of fuzzy-matching "it" against event titles.
    private static let domainSibling: [String: String] = [
        "calendar.cancel_event": "reminder.remove",
        "reminder.remove": "calendar.cancel_event",
    ]

    func propose(_ call: ToolCall, allowChoice: Bool = true) {
        pendingChoice = nil
        var toolID = call.toolID
        var params = call.params
        // "remove it" / "cancel this event": referent phrases resolve against
        // what the conversation just touched or LISTED. One target -> direct;
        // several -> the listing becomes the choice list. A referent NEVER falls
        // through to title matching ("it" must not fuzzy-match "DentIsT").
        if let match = params["match"], EngineBridge.shared.aiIsReferent(match) {
            let domain = toolID.hasPrefix("reminder") ? "reminder" : "calendar"
            var targets = recentTargets.filter { $0.domain == domain }
            if targets.isEmpty, let sibling = Self.domainSibling[toolID] {
                let others = recentTargets.filter { $0.domain != domain }
                if !others.isEmpty {
                    toolID = sibling
                    targets = others
                }
            }
            if targets.count == 1 {
                params["chosen_id"] = targets[0].id
            } else if targets.count > 1 {
                guard allowChoice else { return }
                pending = nil
                pendingChoice = PendingChoice(
                    toolID: toolID, params: params,
                    candidates: targets.map { ActionCandidate(id: $0.id, label: $0.label) })
                feedback = ""
                return
            } else {
                pending = nil
                feedback = "Nothing recent to refer to. Name the item instead."
                return
            }
        }
        switch resolveOutcome(toolID: toolID, params: params) {
        case .planned(let plan):
            pending = plannedAction(toolID: toolID, plan: plan)
            feedback = ""
        case .invalid(let message):
            pending = nil
            feedback = message
        case .choice(let candidates):
            pending = nil
            guard allowChoice else { return }  // live preview stays quiet
            pendingChoice = PendingChoice(
                toolID: toolID, params: params, candidates: candidates)
            feedback = ""
        }
    }

    /// Builds the P4 resolve request (candidates + the NSDataDetector-resolved
    /// date seam) and asks the Rust core for the outcome. Pure CPU + a local
    /// EventKit fetch for mutate tools.
    private func resolveOutcome(toolID: String, params: [String: String]) -> ActionResolveOutcome {
        let now = Date()
        var request: [String: Any] = [
            "tool": toolID,
            "params": params,
            "now": Int64(now.timeIntervalSince1970),
        ]
        // The mutate tools (cancel/move/remove/complete/snooze) resolve against
        // the targets loaded once via `syncAITargets`, so their lists are omitted
        // here; the Rust store supplies them. block_time keeps its own events
        // below (a window-specific, on-Enter fetch).
        if let when = params["when"], let resolved = resolveWhen(when, now: now) {
            let leaning = EngineBridge.shared.aiFutureLeaning(phrase: when, resolved: resolved, now: now)
            request["resolved_when"] = Int64(leaning.timeIntervalSince1970)
        }
        // block_time searches a day-range for a free slot: resolve the window
        // (grammar), pass its events plus the bounds.
        if toolID == "calendar.block_time" {
            let window = params["when"].flatMap { EngineBridge.shared.aiQueryWindow($0) }
            let start = window?.start ?? now
            let end = window?.end ?? now.addingTimeInterval(2 * 86_400)
            request["window_start"] = Int64(start.timeIntervalSince1970)
            request["window_end"] = Int64(end.timeIntervalSince1970)
            request["events"] = EventKitService.shared
                .eventCandidates(from: start, to: end)
                .map { event -> [String: Any] in
                    [
                        "id": event.id, "title": event.title,
                        "start": Int64(event.start.timeIntervalSince1970),
                        "end": Int64(event.end.timeIntervalSince1970),
                        "all_day": event.isAllDay,
                    ]
                }
        }
        guard
            let data = try? JSONSerialization.data(withJSONObject: request),
            let json = String(data: data, encoding: .utf8),
            let outcome = EngineBridge.shared.aiResolve(requestJSON: json)
        else { return .invalid("Resolution failed.") }
        return ActionResolveOutcome.decode(outcome)
    }

    /// Resolves a planner call into a directly runnable action for the main
    /// bar's action row - the visible row IS the confirmation surface, so no
    /// second confirm follows. nil when resolution needs the session
    /// (disambiguation, referents) or fails; those fall back to the AI surface.
    func quickAction(for call: ToolCall) -> PlannedAction? {
        EventKitService.shared.refreshReminderCache()
        EventKitService.shared.refreshEventCache()
        guard case .planned(let plan) = resolveOutcome(toolID: call.toolID, params: call.params)
        else { return nil }
        return plannedAction(toolID: call.toolID, plan: plan)
    }

    /// One-Enter execution of a quickAction: performs, records the receipt so
    /// ⌘Z undoes, returns the summary for the caller's banner.
    func performQuickAction(_ action: PlannedAction) async throws -> String {
        await ensureAccess(for: action.toolID)
        let receipt = try action.perform()
        lastReceipt = receipt
        return receipt.summary
    }

    /// The date seam: NSDataDetector resolves the phrase, the shared-lexicon
    /// day-phrase (Rust core) cross-checks WHICH day. A named day wins - "tue
    /// 9am" typed on a Monday must be Tuesday, not the detector's "today" -
    /// while the detector keeps the time of day. Bare day words the detector
    /// can't read at all ("this week wed") resolve to the named day's midnight
    /// (all-day downstream).
    private func resolveWhen(_ when: String, now: Date) -> Date? {
        let detected = DatePhrase.resolve(when)
        guard let named = EngineBridge.shared.aiDayPhrase(when) else { return detected }
        guard let detected else { return named }
        let calendar = Calendar.current
        if calendar.isDate(detected, inSameDayAs: named) { return detected }
        let time = calendar.dateComponents([.hour, .minute], from: detected)
        return calendar.date(
            bySettingHour: time.hour ?? 0, minute: time.minute ?? 0, second: 0, of: named)
            ?? named
    }

    /// Wraps a Rust plan into the executable action the confirm flow runs.
    private func plannedAction(toolID: String, plan: ActionResolvedPlan) -> PlannedAction {
        PlannedAction(
            toolID: toolID,
            preview: ActionPreview(title: plan.previewTitle, detail: plan.previewDetail)
        ) {
            let newID = try ActionExecutor.perform(plan.execute)
            let subjectID = plan.subject == "new" ? newID : plan.subject
            return ActionReceipt(
                summary: plan.summary,
                subjectID: subjectID,
                undo: ActionExecutor.undoClosure(for: plan.undo, newID: newID))
        }
    }

    /// The user picked from the disambiguation list: re-plan with the exact id.
    func choose(_ candidate: ActionCandidate) {
        guard var choice = pendingChoice else { return }
        choice.params["chosen_id"] = candidate.id
        pendingChoice = nil
        propose(ToolCall(toolID: choice.toolID, params: choice.params))
    }

    /// What "it" / "this event" can refer to: the last confirmed action's
    /// subject, or the events of the last listing shown in this conversation.
    struct RecentTarget {
        let domain: String
        let id: String
        let label: String
    }

    private var recentTargets: [RecentTarget] = []

    /// Confirm the pending action. Requests calendar/reminder access on first use
    /// (the only place a permission prompt appears, besides the Settings button).
    func confirm() {
        guard let action = pending else { return }
        pending = nil
        Task {
            await ensureAccess(for: action.toolID)
            do {
                let receipt = try action.perform()
                lastReceipt = receipt
                if let subjectID = receipt.subjectID {
                    recentTargets = [RecentTarget(
                        domain: action.toolID.hasPrefix("reminder") ? "reminder" : "calendar",
                        id: subjectID,
                        label: receipt.summary)]
                }
                undoableItemID = chat.appendAction(receipt.summary)
                feedback = ""
            } catch {
                feedback = "Failed: \(error.localizedDescription)"
            }
        }
    }

    func cancel() {
        planTask?.cancel()
        idleTask?.cancel()
        chat.cancelStreaming()
        planGeneration += 1
        pending = nil
        pendingChoice = nil
        feedback = ""
        isPlanning = false
    }

    /// The user emptied the compose input by hand: cancel an in-flight
    /// preview/plan. A submit's own input-clear never reaches this - the view
    /// clears through `clearQuerySilently`, which skips these side effects.
    func handleComposeCleared() {
        idleTask?.cancel()
        if isPresenting || isPlanning {
            cancel()
        }
    }

    func undoLast() {
        guard let receipt = lastReceipt else { return }
        do {
            try receipt.undo()
            if let id = undoableItemID {
                chat.annotate(id, suffix: "  ·  undone")
            }
            feedback = ""
        } catch {
            feedback = "Undo failed"
        }
        lastReceipt = nil
        undoableItemID = nil
    }

    /// Prompt for ONLY the store this action touches: a reminder must not pop a
    /// Calendar dialog first (TCC prompts are one-shot, so a denial there is
    /// permanent and unrelated to what the user asked for).
    private func ensureAccess(for toolID: String) async {
        let isReminder = toolID.hasPrefix("reminder")
        let access = isReminder
            ? EventKitService.shared.reminderAccess
            : EventKitService.shared.calendarAccess
        guard access == .notDetermined else { return }
        if isReminder {
            await EventKitService.shared.requestReminderAccess()
        } else {
            await EventKitService.shared.requestCalendarAccess()
        }
    }
}
