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
        TextOpSource.resolve(mentioned: attachments.files, pickedFilePaths: pickedFilePaths)
    }

    /// The plan awaiting confirm: one step for most requests, several for a
    /// compound one ("cancel the dentist and move lunch to 1pm"). Confirmed
    /// with a single Enter and undone as a unit, so a compound request cannot
    /// leave half of itself behind with no way back.
    @Published private(set) var pendingSteps: [PlannedAction] = []
    @Published private(set) var pendingChoice: PendingChoice?
    /// Receipts from the last confirmed plan, in the order they ran. Undo
    /// reverses them back to front.
    @Published private(set) var lastReceipts: [ActionReceipt] = []
    var lastReceipt: ActionReceipt? { lastReceipts.last }
    @Published private(set) var feedback: String = ""
    /// The action item the current `lastReceipt` can undo (answers may stack
    /// after it, so "last item" is not reliable).
    @Published private(set) var undoableItemIDs: [UUID] = []

    /// Whether this transcript item is part of the plan undo would reverse.
    func canUndo(_ itemID: UUID) -> Bool {
        !lastReceipts.isEmpty && undoableItemIDs.contains(itemID)
    }
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

    var isPresenting: Bool { !pendingSteps.isEmpty }

    /// A post-action result (success summary + undo, or an error message) is
    /// showing. Dismissed when the user types something new or hides the window.
    var hasResult: Bool { !lastReceipts.isEmpty || !feedback.isEmpty }

    var sessionActive: Bool { chat.isActive }

    func dismissResult() {
        lastReceipts = []
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
            pendingSteps = [plannedAction(toolID: raw.toolID, plan: plan)]
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
            pendingSteps = []
            isPlanning = false
            feedback = ""
            return
        }
        pendingSteps = []
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
            let calls = await self.planner.plan(query: query)
            guard generation == self.planGeneration else { return }  // superseded
            self.isPlanning = false
            if !calls.isEmpty, !calls.contains(where: Self.isUtilityCall) {
                // Live preview never pops a choice list mid-typing; Enter does.
                // Utility calls (recall/textop) only ever run on Enter, so a
                // plan containing one previews nothing.
                self.propose(calls, allowChoice: false)
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
        // One message, one turn: whichever route handles it claims the same
        // transcript entry instead of each path deciding for itself.
        chat.startTurn()

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
            pendingSteps = []
            feedback = memoryFeedback
        case .textOp(let label, let instruction):
            isPlanning = false
            pendingSteps = []
            if let note = chat.runTextOp(
                label: label, instruction: instruction, source: textOpSource()) {
                feedback = note
            }
        case .files:
            isPlanning = false
            pendingSteps = []
            // Empty params = "parse the raw query"; the shell owns results.
            requestRecall(query: query, params: [:])
        case .explicit(let toolID, let params):
            if case .planned(let plan) = resolveOutcome(toolID: toolID, params: params) {
                isPlanning = false
                pendingSteps = [plannedAction(toolID: toolID, plan: plan)]
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
        chat.beginTurn(text: query, attachedPaths: attachments.paths)
        let generation = planGeneration
        planTask = Task { [weak self] in
            guard let self else { return }
            let calls = await self.planner.plan(query: query)
            guard generation == self.planGeneration else { return }  // superseded
            self.isPlanning = false
            if calls.count > 1 {
                // A compound request: preview every step, confirm once.
                self.propose(calls)
            } else if let call = calls.first {
                self.handlePlannedCall(call, rawQuery: query)
            } else {
                // Not an add-action: treat it as a chat turn in the session.
                self.chat.askChat(query, attachments: self.attachments)
            }
        }
    }

    /// A recall request, unless a file is attached - then the user is asking
    /// ABOUT that file, not searching for others. "summary this file please?"
    /// contains the word "file", so the deterministic parser claims it and the
    /// shell would leave AI mode to show a file LIST, discarding the question.
    /// The parser cannot know about attachments; only this layer does, so the
    /// rule lives here once rather than at each recall entry point.
    private func requestRecall(query: String, params: [String: String]) {
        guard attachments.isEmpty else {
            chat.askChat(query, attachments: attachments)
            return
        }
        recallRequest = RecallRequest(query: query, params: params)
    }

    /// Which store a tool touches. Spelled out at three call sites before.
    private static func domain(of toolID: String) -> String {
        toolID.hasPrefix("reminder") ? "reminder" : "calendar"
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
            requestRecall(query: rawQuery, params: call.params)
        case "clipboard.textop":
            guard let instruction = call.params["instruction"], !instruction.isEmpty else {
                chat.askChat(rawQuery, attachments: attachments)
                return
            }
            let label = String(instruction.prefix(48))
            if let note = chat.runTextOp(
                label: label, instruction: instruction, source: textOpSource()) {
                feedback = note
            }
        default:
            propose(call)
        }
    }

    /// A chat turn in the session: the question and a streaming answer stack as
    /// items, with the session (including performed actions) as conversation
    /// context. This is what a non-action `>` query becomes on Enter.
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

    /// A whole plan. Every step must resolve: a compound request that can only
    /// half-execute is refused with the reason, rather than previewing the part
    /// that happened to work and silently dropping the rest.
    func propose(_ calls: [ToolCall], allowChoice: Bool = true) {
        guard calls.count > 1 else {
            if let single = calls.first { propose(single, allowChoice: allowChoice) }
            return
        }
        pendingChoice = nil
        var steps: [PlannedAction] = []
        for (index, call) in calls.enumerated() {
            // Referents resolve per step: "cancel the dentist and move it to
            // 1pm" carries match "it" on step two.
            let toolID: String
            let params: [String: String]
            switch resolveReferent(toolID: call.toolID, params: call.params) {
            case .resolved(let resolvedTool, let resolvedParams):
                toolID = resolvedTool
                params = resolvedParams
            case .several:
                // A choice list mid-plan would need a queue of questions.
                pendingSteps = []
                feedback = "Step \(index + 1) refers to more than one recent item - "
                    + "name it instead."
                return
            case .nothingToReferTo:
                pendingSteps = []
                feedback = "Step \(index + 1) refers to nothing recent. Name the item instead."
                return
            }
            switch resolveOutcome(toolID: toolID, params: params) {
            case .planned(let plan):
                steps.append(plannedAction(toolID: toolID, plan: plan))
            case .invalid(let message):
                pendingSteps = []
                feedback = "Step \(index + 1): \(message)"
                return
            case .choice:
                // Disambiguating one step of several would need a queue of
                // questions; say which step is unclear instead of guessing.
                pendingSteps = []
                feedback = "Step \(index + 1) matches more than one thing - "
                    + "name it more precisely."
                return
            }
        }
        pendingSteps = steps
        feedback = ""
    }

    /// What a referent phrase resolved to. Shared by the single-step and
    /// compound paths: a compound plan that skipped this would let "move it to
    /// 1pm" fuzzy-match "it" against event titles, which is exactly what the
    /// referent rule exists to prevent.
    enum ReferentOutcome {
        /// No referent, or one resolved to a single target.
        case resolved(toolID: String, params: [String: String])
        /// Several recent targets; the caller decides whether to offer a list.
        case several(toolID: String, params: [String: String], targets: [RecentTarget])
        case nothingToReferTo
    }

    /// "remove it" / "cancel this event": referent phrases resolve against what
    /// the conversation just touched or LISTED. A referent NEVER falls through
    /// to title matching ("it" must not fuzzy-match "DentIsT").
    private func resolveReferent(toolID: String, params: [String: String]) -> ReferentOutcome {
        var toolID = toolID
        var params = params
        guard let match = params["match"], EngineBridge.shared.aiIsReferent(match) else {
            return .resolved(toolID: toolID, params: params)
        }
        let domain = Self.domain(of: toolID)
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
            return .resolved(toolID: toolID, params: params)
        }
        if targets.count > 1 {
            return .several(toolID: toolID, params: params, targets: targets)
        }
        return .nothingToReferTo
    }

    func propose(_ call: ToolCall, allowChoice: Bool = true) {
        pendingChoice = nil
        let toolID: String
        let params: [String: String]
        switch resolveReferent(toolID: call.toolID, params: call.params) {
        case .resolved(let resolvedTool, let resolvedParams):
            toolID = resolvedTool
            params = resolvedParams
        case .several(let resolvedTool, let resolvedParams, let targets):
            guard allowChoice else { return }
            pendingSteps = []
            pendingChoice = PendingChoice(
                toolID: resolvedTool, params: resolvedParams,
                candidates: targets.map { ActionCandidate(id: $0.id, label: $0.label) })
            feedback = ""
            return
        case .nothingToReferTo:
            pendingSteps = []
            feedback = "Nothing recent to refer to. Name the item instead."
            return
        }
        switch resolveOutcome(toolID: toolID, params: params) {
        case .planned(let plan):
            pendingSteps = [plannedAction(toolID: toolID, plan: plan)]
            feedback = ""
        case .invalid(let message):
            pendingSteps = []
            feedback = message
        case .choice(let candidates):
            pendingSteps = []
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
        lastReceipts = [receipt]
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
    /// Runs the confirmed plan in order. A step that throws STOPS the run and
    /// says which one: the steps that already ran keep their receipts, so undo
    /// still reverses exactly what happened rather than pretending the whole
    /// plan failed or silently carrying on past a failure.
    func confirm() {
        let steps = pendingSteps
        guard !steps.isEmpty else { return }
        pendingSteps = []
        Task {
            var receipts: [ActionReceipt] = []
            var itemIDs: [UUID] = []
            var targets: [RecentTarget] = []
            for (index, step) in steps.enumerated() {
                await ensureAccess(for: step.toolID)
                do {
                    let receipt = try step.perform()
                    receipts.append(receipt)
                    if let subjectID = receipt.subjectID {
                        targets.append(RecentTarget(
                            domain: Self.domain(of: step.toolID),
                            id: subjectID,
                            label: receipt.summary))
                    }
                    itemIDs.append(chat.appendAction(receipt.summary))
                    feedback = ""
                } catch {
                    feedback = steps.count == 1
                        ? "Failed: \(error.localizedDescription)"
                        : "Step \(index + 1) of \(steps.count) failed: "
                            + "\(error.localizedDescription)"
                    break
                }
            }
            lastReceipts = receipts
            undoableItemIDs = itemIDs
            if !targets.isEmpty { recentTargets = targets }
        }
    }

    func cancel() {
        planTask?.cancel()
        idleTask?.cancel()
        chat.cancelStreaming()
        planGeneration += 1
        pendingSteps = []
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

    /// Reverses the last plan back to front, so a compound request undoes as
    /// one thing. A step that fails to reverse is reported but does not strand
    /// the others: the rest still get undone.
    func undoLast() {
        guard !lastReceipts.isEmpty else { return }
        var failures = 0
        for (index, receipt) in lastReceipts.enumerated().reversed() {
            do {
                try receipt.undo()
                if index < undoableItemIDs.count {
                    chat.annotate(undoableItemIDs[index], suffix: "  ·  undone")
                }
            } catch {
                failures += 1
            }
        }
        feedback = failures == 0
            ? ""
            : (failures == lastReceipts.count
                ? "Undo failed"
                : "Undo failed for \(failures) of \(lastReceipts.count) steps")
        lastReceipts = []
        undoableItemIDs = []
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
