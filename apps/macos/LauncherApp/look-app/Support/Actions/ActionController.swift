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
    /// Parsed markdown, computed on text/kind change so the view renders without
    /// re-parsing (FFI + full-text scan) on every frame. Empty for non-answers.
    private(set) var segments: [EngineBridge.AIMarkdownSegment] = []

    init(kind: Kind = .action, text: String, source: String? = nil) {
        self.kind = kind
        self.text = text
        self.source = source
        recomputeSegments()
    }

    private mutating func recomputeSegments() {
        segments = kind == .answer ? EngineBridge.shared.aiMarkdownSegments(text) : []
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

    @Published private(set) var pending: PlannedAction?
    @Published private(set) var pendingChoice: PendingChoice?
    @Published private(set) var lastReceipt: ActionReceipt?
    @Published private(set) var feedback: String = ""
    /// Completed actions this session, shown as a stack in the AI panel. The
    /// session (and the stack) ends on Esc, hide, or moving on to normal search.
    @Published private(set) var sessionItems: [ActionSessionItem] = []
    /// The action item the current `lastReceipt` can undo (answers may stack
    /// after it, so "last item" is not reliable).
    @Published private(set) var undoableItemID: UUID?
    /// True while the model is turning a `>` query into an action, so the UI can
    /// show a "thinking" indicator during the generation.
    @Published private(set) var isPlanning: Bool = false

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

    var sessionActive: Bool { !sessionItems.isEmpty }

    func dismissResult() {
        lastReceipt = nil
        feedback = ""
    }

    /// Dismiss a transient result line (e.g. "Remembered …") on the next input.
    func clearFeedback() {
        if !feedback.isEmpty { feedback = "" }
    }

    /// Close the current conversation: save it, drop pending work, clear the
    /// stack, and mint a fresh conversation id. Fired by Esc with nothing
    /// pending (leaving AI mode).
    func endSession() {
        saveConversation()
        cancel()
        sessionItems.removeAll()
        lastReceipt = nil
        undoableItemID = nil
        recentTargets = []
        conversationID = UUID()
    }

    /// Restore a stored conversation to continue it. The full transcript shows;
    /// the model context stays capped (last 10 items) so continuing an old
    /// conversation carries reasonable, bounded weight.
    func continueConversation(_ conversation: AIConversation) {
        endSession()
        conversationID = conversation.id
        sessionItems = conversation.items.map { stored in
            ActionSessionItem(
                kind: ActionSessionItem.Kind(rawValue: stored.kind) ?? .answer,
                text: stored.text,
                source: stored.source)
        }
    }

    private var conversationID = UUID()

    /// Upserts the live conversation into the capped store. Called whenever an
    /// item completes, so quitting mid-session loses nothing that finished.
    private func saveConversation() {
        let items = sessionItems.filter { item in
            // Skip an answer that never got content (placeholder only).
            !(item.kind == .answer
                && (item.text == "…"
                    || item.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty))
        }
        guard !items.isEmpty else { return }
        ConversationStore.upsert(AIConversation(
            id: conversationID,
            title: String((items.first?.text ?? "Conversation").prefix(48)),
            updatedAt: Date(),
            items: items.map {
                AIConversation.StoredItem(kind: $0.kind.rawValue, text: $0.text, source: $0.source)
            }))
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
            if let call {
                // Live preview never pops a choice list mid-typing; Enter does.
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

        // Explicit memory command ("remember …", "forget …", "memories"):
        // deterministic, never touches the model.
        if let memoryFeedback = MemoryStore.command(query) {
            isPlanning = false
            pending = nil
            feedback = memoryFeedback
            return
        }

        // Clipboard text-op verb ("summarize", "fix grammar", "translate to …"):
        // deterministic routing, transforms whatever text is on the clipboard.
        if let op = EngineBridge.shared.aiTextOp(input: query) {
            isPlanning = false
            pending = nil
            runTextOp(label: op.label, instruction: op.instruction)
            return
        }

        EventKitService.shared.refreshReminderCache()
        EventKitService.shared.refreshEventCache()

        let modelAvailable = planner.isAvailable
        if let raw = EngineBridge.shared.aiParseExplicit(">" + query, modelAvailable: modelAvailable),
           case .planned(let plan) = resolveOutcome(toolID: raw.toolID, params: raw.params) {
            isPlanning = false
            pending = plannedAction(toolID: raw.toolID, plan: plan)
            feedback = ""
            return
        }

        guard modelAvailable else {
            // No planner: hand the text to the provider's answer path instead of
            // dead-ending (askChat itself reports if no provider is usable).
            askChat(query)
            return
        }
        // The message joins the transcript NOW; the planner round-trip ("is
        // this an action?") happens under the Thinking indicator, not before
        // the user sees their own message.
        isPlanning = true
        sessionItems.append(ActionSessionItem(kind: .user, text: query))
        saveConversation()
        let generation = planGeneration
        planTask = Task { [weak self] in
            guard let self else { return }
            let call = await self.planner.plan(query: query)
            guard generation == self.planGeneration else { return }  // superseded
            self.isPlanning = false
            if let call {
                self.propose(call)
            } else {
                // Not an add-action: treat it as a chat turn in the session.
                self.askChat(query, userItemAppended: true)
            }
        }
    }

    /// A chat turn in the session: the question and a streaming answer stack as
    /// items, with the session (including performed actions) as conversation
    /// context. This is what a non-action `>` query becomes on Enter.
    /// `userItemAppended` = the submit already put the message in the transcript
    /// (it shows instantly while the planner thinks); don't add it twice.
    func askChat(_ query: String, userItemAppended: Bool = false) {
        chatTask?.cancel()
        // A leftover disambiguation list must not survive into an unrelated
        // turn: a later bare number would still fire the old (possibly
        // destructive) choice.
        pendingChoice = nil
        if !userItemAppended {
            sessionItems.append(ActionSessionItem(kind: .user, text: query))
            saveConversation()
        }

        // Schedule-sounding questions get the calendar as context, so "what's my
        // next meeting" / "am I free friday" just answer. The data only ever
        // goes to the on-machine model. Without read access, the model is told
        // to point at Settings instead of failing mysteriously.
        let scheduleContext: String? = {
            guard Self.mentionsSchedule(query) else { return nil }
            guard let (summary, label) = scheduleSummary(for: query) else {
                return "You cannot see the user's calendar because access is not "
                    + "granted. Tell them to connect Calendar via the Permissions "
                    + "row in Look's Settings."
            }
            let df = DateFormatter()
            df.dateFormat = "EEEE, MMMM d yyyy, HH:mm"
            return "Now: \(df.string(from: Date())). "
                + "The user's calendar \(label):\n\(summary)"
        }()

        // Long-term memory (facts the user asked to remember), injected on every
        // turn so the assistant "knows them" across conversations.
        let memoryContext = MemoryStore.context()

        var messages: [[String: String]] = [
            ["role": "system", "content": Self.chatInstructions]
        ]
        // Injected after the static prompt so the prompt-cache prefix holds.
        if !memoryContext.isEmpty {
            messages.append(["role": "system", "content": memoryContext])
        }
        if let scheduleContext {
            messages.append(["role": "system", "content": scheduleContext])
        }
        // Token-budget window (not a fixed count): keep the newest turns that fit
        // ~2.5k tokens, so long chats stay coherent without a summarizer.
        let itemTexts = sessionItems.map { item -> String in
            item.kind == .action ? "[Done: \(item.text)]" : item.text
        }
        let keep = EngineBridge.shared.aiContextWindow(texts: itemTexts, budget: 0)
        for item in sessionItems.suffix(keep) {
            switch item.kind {
            case .user:
                messages.append(["role": "user", "content": item.text])
            case .answer:
                if !item.text.isEmpty, item.text != "…" {
                    messages.append(["role": "assistant", "content": item.text])
                }
            case .action:
                messages.append(["role": "assistant", "content": "[Done: \(item.text)]"])
            }
        }

        let settings = ThemeStore.shared.settings
        let sourceLabel = settings.aiProvider == .ollama
            ? settings.ollamaModel
            : settings.aiProvider.title
        let placeholder = ActionSessionItem(kind: .answer, text: "…", source: sourceLabel)
        sessionItems.append(placeholder)
        let placeholderID = placeholder.id

        // Ollama gets the full session as context; any other provider (e.g.
        // Apple Intelligence) answers single-turn through the router (with the
        // schedule context folded into the prompt), so `>` chat works on-device.
        let provider = settings.aiProvider
        let preamble = [memoryContext.isEmpty ? nil : memoryContext, scheduleContext]
            .compactMap { $0 }.joined(separator: "\n\n")
        let routed = preamble.isEmpty ? query : "\(preamble)\n\nUser question: \(query)"
        let host = settings.ollamaHost
        let model = settings.ollamaModel

        // Ollama: streamed through the Rust core (curl child + polling), the
        // same transport linows will use. Other providers keep their native
        // single-turn stream via the router.
        if provider == .ollama {
            if let messagesData = try? JSONSerialization.data(withJSONObject: messages),
               let messagesJSON = String(data: messagesData, encoding: .utf8) {
                let session = EngineBridge.shared.aiChatStart(
                    host: host, model: model, messagesJSON: messagesJSON)
                if session != 0 {
                    chatTask = Task { [weak self] in
                        await self?.consumeChatSession(session, into: placeholderID)
                    }
                    return
                }
            }
            updateItem(placeholderID, text: chatFailureMessage())
            saveConversation()
            return
        }

        let makeStream: @MainActor () -> AsyncThrowingStream<String, Error>? = {
            AIQueryRouter.shared.answer(query: routed, using: provider)
        }

        if let stream = makeStream() {
            chatTask = Task { [weak self] in await self?.consume(stream, into: placeholderID) }
            return
        }

        // Schedule questions don't actually need a model: answer with the
        // deterministic listing.
        if Self.mentionsSchedule(query), let (summary, label) = scheduleSummary(for: query) {
            updateItem(placeholderID, text: "Your calendar \(label):\n\(summary)", source: "Calendar")
            saveConversation()
            return
        }

        // FoundationModels can report unavailable right after app launch until
        // first touched; the prewarm above touches it, so retry briefly before
        // surfacing the real reason.
        chatTask = Task { [weak self] in
            for delaySeconds in [1.0, 2.0] {
                try? await Task.sleep(nanoseconds: UInt64(delaySeconds * 1_000_000_000))
                guard let self, !Task.isCancelled else { return }
                if let stream = makeStream() {
                    await self.consume(stream, into: placeholderID)
                    return
                }
            }
            guard let self, !Task.isCancelled else { return }
            var reason = ""
            if case .unavailable(let r) = AIQueryRouter.shared.availability(of: provider) {
                reason = " " + r.userFacingMessage
            }
            self.updateItem(
                placeholderID,
                text: "No model available.\(reason) Or use: >add <title> @ <time>")
            self.saveConversation()
        }
    }

    /// A clipboard text-op: transform whatever text is on the clipboard with the
    /// given instruction, streaming the result into the panel (reusing chat).
    func runTextOp(label: String, instruction: String) {
        let clip = (NSPasteboard.general.string(forType: .string) ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clip.isEmpty else {
            pending = nil
            feedback = "Copy some text first, then try \"\(label.lowercased())\"."
            return
        }
        chatTask?.cancel()
        sessionItems.append(ActionSessionItem(kind: .user, text: label))
        saveConversation()

        let settings = ThemeStore.shared.settings
        let sourceLabel = settings.aiProvider == .ollama
            ? settings.ollamaModel
            : settings.aiProvider.title
        let placeholder = ActionSessionItem(kind: .answer, text: "…", source: sourceLabel)
        sessionItems.append(placeholder)
        let placeholderID = placeholder.id

        let messages: [[String: String]] = [
            ["role": "system", "content": instruction],
            ["role": "user", "content": clip],
        ]

        if settings.aiProvider == .ollama {
            if let data = try? JSONSerialization.data(withJSONObject: messages),
               let json = String(data: data, encoding: .utf8) {
                let session = EngineBridge.shared.aiChatStart(
                    host: settings.ollamaHost, model: settings.ollamaModel, messagesJSON: json)
                if session != 0 {
                    chatTask = Task { [weak self] in
                        await self?.consumeChatSession(session, into: placeholderID)
                    }
                    return
                }
            }
            updateItem(placeholderID, text: chatFailureMessage())
            saveConversation()
            return
        }

        let routed = "\(instruction)\n\n\(clip)"
        if let stream = AIQueryRouter.shared.answer(query: routed, using: settings.aiProvider) {
            chatTask = Task { [weak self] in await self?.consume(stream, into: placeholderID) }
            return
        }
        updateItem(placeholderID, text: chatFailureMessage())
        saveConversation()
    }

    /// Polls a Rust-core chat session (~12x/sec) into the answer item, then
    /// saves. Cancellation aborts the curl child, stopping generation.
    private func consumeChatSession(_ session: UInt64, into id: UUID) async {
        defer {
            if Task.isCancelled { EngineBridge.shared.aiChatCancel(session) }
        }
        while !Task.isCancelled {
            try? await Task.sleep(nanoseconds: 85_000_000)
            if Task.isCancelled { return }
            guard let snapshot = EngineBridge.shared.aiChatPoll(session) else { return }
            if !snapshot.text.isEmpty {
                updateItem(id, text: snapshot.text)
            }
            if let error = snapshot.error {
                // Mid-stream failures keep the partial answer, with the reason.
                let message = chatFailureMessage(detail: error)
                updateItem(
                    id,
                    text: snapshot.text.isEmpty ? message : snapshot.text + "\n\n" + message)
            } else if snapshot.truncated == true, !snapshot.text.isEmpty {
                updateItem(id, text: snapshot.text + "\n\n(Answer cut off at the length limit.)")
            }
            if snapshot.done {
                saveConversation()
                return
            }
        }
    }

    /// Streams cumulative snapshots into the answer item, then archives it.
    private func consume(_ stream: AsyncThrowingStream<String, Error>, into id: UUID) async {
        do {
            for try await partial in stream {
                if Task.isCancelled { return }
                updateItem(id, text: partial)
            }
        } catch {
            if !Task.isCancelled {
                let detail = (error as? OllamaError)?.errorDescription
                updateItem(id, text: chatFailureMessage(detail: detail))
            }
        }
        if !Task.isCancelled { saveConversation() }
    }

    /// The most specific failure text available: the server's own message when
    /// the stream carried one ("model 'x' not found"), else the health probe's
    /// actionable diagnosis ("Ollama is not running. Start it with: ollama
    /// serve"), else the generic fallback.
    private func chatFailureMessage(detail: String? = nil) -> String {
        if let detail, !detail.isEmpty, detail != "no response" {
            return "Answer failed: \(detail)"
        }
        let provider = ThemeStore.shared.settings.aiProvider
        if case .unavailable(let reason) = AIQueryRouter.shared.availability(of: provider) {
            return reason.userFacingMessage
        }
        return "Answer failed. Is the model available?"
    }

    private func updateItem(_ id: UUID, text: String, source: String? = nil) {
        guard let idx = sessionItems.firstIndex(where: { $0.id == id }) else { return }
        sessionItems[idx].text = text
        if let source { sessionItems[idx].source = source }
    }

    /// Calendar/reminder listing scoped to what the question names. Returns the
    /// text plus a human label ("for next week").
    private func scheduleSummary(for query: String) -> (String, String)? {
        // A reminder-specific question lists reminders, not events.
        if Self.mentionsReminders(query), !Self.mentionsEvents(query) {
            guard let (summary, reminders) = EventKitService.shared.remindersSummary() else {
                return nil
            }
            rememberListedReminders(reminders)
            return (summary, "reminders")
        }
        if let window = EngineBridge.shared.aiQueryWindow(query) {
            guard let (summary, events) = EventKitService.shared.eventsSummary(
                from: window.start, to: window.end,
                emptyText: "No events \(window.label).")
            else { return nil }
            rememberListedEvents(events)
            return (summary, "for \(window.label)")
        }
        guard let (summary, events) = EventKitService.shared.upcomingEventsSummary() else { return nil }
        rememberListedEvents(events)
        return (summary, "for the next 7 days")
    }

    /// A listing shown to the user becomes the referent set: "remove this
    /// event" right after "what's on this week?" targets what was listed.
    private func rememberListedEvents(_ events: [EventCandidateData]) {
        guard !events.isEmpty else { return }
        recentTargets = events.map {
            RecentTarget(
                domain: "calendar", id: $0.id,
                label: "\($0.title)  ·  \(DatePhrase.format($0.start))")
        }
    }

    private func rememberListedReminders(_ reminders: [ReminderCandidateData]) {
        guard !reminders.isEmpty else { return }
        recentTargets = reminders.map {
            RecentTarget(domain: "reminder", id: $0.id, label: $0.title)
        }
    }

    private static let reminderWords: Set<String> = ["reminder", "reminders", "todo", "todos", "tasks"]
    private static let eventWords: Set<String> = ["event", "events", "meeting", "meetings", "calendar", "appointment", "appointments"]

    private static func mentionsReminders(_ query: String) -> Bool { hasWord(query, reminderWords) }
    private static func mentionsEvents(_ query: String) -> Bool { hasWord(query, eventWords) }
    private static func hasWord(_ query: String, _ set: Set<String>) -> Bool {
        query.lowercased().split(whereSeparator: { !$0.isLetter }).contains { set.contains(String($0)) }
    }

    /// Cheap word gate for injecting calendar context into a chat turn.
    private static let scheduleWords: Set<String> = [
        "meeting", "meetings", "event", "events", "calendar", "schedule",
        "scheduled", "free", "busy", "appointment", "appointments", "reminder",
        "reminders", "today", "tomorrow", "tonight", "week", "weekend", "next",
        "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
    ]

    private static func mentionsSchedule(_ query: String) -> Bool {
        query.lowercased()
            .split(whereSeparator: { !$0.isLetter })
            .contains { scheduleWords.contains(String($0)) }
    }

    private static let chatInstructions = """
        You are Look's built-in assistant on macOS. Be concise and helpful. \
        Plain text; short code snippets are fine when asked. You cannot modify \
        or delete calendar items; adding events and reminders happens outside \
        this chat, so if asked to change or remove one, say it isn't supported \
        yet.
        """

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
        if let when = params["when"], let resolved = DatePhrase.resolve(when, now: now) {
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
                let item = ActionSessionItem(text: receipt.summary)
                sessionItems.append(item)
                undoableItemID = item.id
                saveConversation()
                feedback = ""
            } catch {
                feedback = "Failed: \(error.localizedDescription)"
            }
        }
    }

    func cancel() {
        planTask?.cancel()
        idleTask?.cancel()
        chatTask?.cancel()
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
            if let idx = sessionItems.firstIndex(where: { $0.id == undoableItemID }) {
                sessionItems[idx].text += "  ·  undone"
                saveConversation()
            }
            feedback = ""
        } catch {
            feedback = "Undo failed"
        }
        lastReceipt = nil
        undoableItemID = nil
    }

    private func ensureAccess(for toolID: String) async {
        let access = toolID.hasPrefix("reminder")
            ? EventKitService.shared.reminderAccess
            : EventKitService.shared.calendarAccess
        if access == .notDetermined {
            await EventKitService.shared.requestAccess()
        }
    }
}
