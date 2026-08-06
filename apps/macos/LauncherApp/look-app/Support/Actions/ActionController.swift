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
    var kind: Kind = .action
    var text: String
    /// Who produced an answer (model tag, "Apple Intelligence", "Calendar"),
    /// captured at creation so the chat header stays truthful per message.
    var source: String? = nil
}

@MainActor
final class ActionController: ObservableObject {
    static let shared = ActionController()

    /// A disambiguation in progress: the tool call awaiting the user's pick.
    struct PendingChoice {
        let toolID: String
        var params: [String: AIValue]
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

    private let registry: ActionRegistry
    private let planner: ActionPlanner
    private var planTask: Task<Void, Never>?
    private var idleTask: Task<Void, Never>?
    private var chatTask: Task<Void, Never>?
    private var lastWarm = Date.distantPast

    /// How long the user must stop typing before the model runs. Short, because
    /// an in-flight call is cancelled cleanly on the next keystroke (Task cancel
    /// closes the connection; Ollama aborts generation on disconnect). The
    /// working indicator shows instantly regardless.
    private static let idleDelay: UInt64 = 300_000_000

    private init() {
        let registry = ActionRegistry()
        registry.register(CalendarAddEventTool(store: EventKitService.shared))
        registry.register(ReminderAddTool(store: EventKitService.shared))
        registry.register(CalendarCancelEventTool(store: EventKitService.shared))
        registry.register(CalendarMoveEventTool(store: EventKitService.shared))
        registry.register(ReminderCompleteTool(store: EventKitService.shared))
        self.registry = registry
        self.planner = ActionPlanner(registry: registry)
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
        idleTask?.cancel()
        planTask?.cancel()
        let query = stripPrefix(rawQuery)

        guard !query.isEmpty else {
            if isPresenting || isPlanning { cancel() }
            return
        }

        // Explicit `@` form: instant, exact, no model. Without a planner the
        // parser runs lenient (whole-phrase date resolution), so `@` entries
        // stay fully functional on any provider.
        let modelAvailable = planner.isAvailable
        if let call = ExplicitActionParser.parse(">" + query, modelAvailable: modelAvailable),
           case .planned(let action) = registry.plan(call, now: Date()) {
            isPlanning = false
            pending = action
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
        idleTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.idleDelay)
            guard let self, !Task.isCancelled else { return }
            let call = await self.planner.plan(query: query)
            guard !Task.isCancelled else { return }  // superseded: newer task owns state
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
        idleTask?.cancel()
        let query = stripPrefix(rawQuery)
        guard !query.isEmpty else { return }
        EventKitService.shared.refreshReminderCache()

        let modelAvailable = planner.isAvailable
        if let call = ExplicitActionParser.parse(">" + query, modelAvailable: modelAvailable),
           case .planned(let action) = registry.plan(call, now: Date()) {
            planTask?.cancel()
            isPlanning = false
            pending = action
            feedback = ""
            return
        }

        guard modelAvailable else {
            // No planner: hand the text to the provider's answer path instead of
            // dead-ending (askChat itself reports if no provider is usable).
            askChat(query)
            return
        }
        planTask?.cancel()
        planTask = Task { [weak self] in
            guard let self else { return }
            self.isPlanning = true
            let call = await self.planner.plan(query: query)
            guard !Task.isCancelled else {
                self.isPlanning = false
                return
            }
            self.isPlanning = false
            if let call {
                self.propose(call)
            } else {
                // Not an add-action: treat it as a chat turn in the session.
                self.askChat(query)
            }
        }
    }

    /// A chat turn in the session: the question and a streaming answer stack as
    /// items, with the session (including performed actions) as conversation
    /// context. This is what a non-action `>` query becomes on Enter.
    func askChat(_ query: String) {
        chatTask?.cancel()
        let userItem = ActionSessionItem(kind: .user, text: query)
        sessionItems.append(userItem)
        saveConversation()

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

        var messages: [[String: String]] = [
            ["role": "system", "content": Self.chatInstructions]
        ]
        // Injected after the static prompt so the prompt-cache prefix holds.
        if let scheduleContext {
            messages.append(["role": "system", "content": scheduleContext])
        }
        for item in sessionItems.suffix(10) {
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
        let routed = scheduleContext.map { "\($0)\n\nUser question: \(query)" } ?? query
        let host = settings.ollamaHost
        let model = settings.ollamaModel
        let makeStream: @MainActor () -> AsyncThrowingStream<String, Error>? = {
            if provider == .ollama {
                return OllamaProvider.chatStream(host: host, model: model, messages: messages)
            }
            return AIQueryRouter.shared.answer(query: routed, using: provider)
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

    /// Streams cumulative snapshots into the answer item, then archives it.
    private func consume(_ stream: AsyncThrowingStream<String, Error>, into id: UUID) async {
        do {
            for try await partial in stream {
                if Task.isCancelled { return }
                updateItem(id, text: partial)
            }
        } catch {
            if !Task.isCancelled {
                updateItem(id, text: "Answer failed. Is the model available?")
            }
        }
        if !Task.isCancelled { saveConversation() }
    }

    private func updateItem(_ id: UUID, text: String, source: String? = nil) {
        guard let idx = sessionItems.firstIndex(where: { $0.id == id }) else { return }
        sessionItems[idx].text = text
        if let source { sessionItems[idx].source = source }
    }

    /// Calendar listing scoped to the timeframe the question names ("next
    /// week", "tomorrow", "friday"), defaulting to the next 7 days. Returns the
    /// text plus a human label ("for next week").
    private func scheduleSummary(for query: String) -> (String, String)? {
        if let window = DatePhrase.queryWindow(for: query, now: Date()) {
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

    func propose(_ call: ToolCall, allowChoice: Bool = true) {
        pendingChoice = nil
        var params = call.params
        // "remove it" / "cancel this event": referent phrases resolve against
        // what the conversation just touched or LISTED. One target -> direct;
        // several -> the listing becomes the choice list; none -> normal match.
        if let match = params["match"]?.stringValue, ReferentPhrase.isReferent(match) {
            let domain = call.toolID.hasPrefix("reminder") ? "reminder" : "calendar"
            let targets = recentTargets.filter { $0.domain == domain }
            if targets.count == 1 {
                params["chosen_id"] = .string(targets[0].id)
            } else if targets.count > 1 {
                guard allowChoice else { return }
                pending = nil
                pendingChoice = PendingChoice(
                    toolID: call.toolID, params: params,
                    candidates: targets.map { ActionCandidate(id: $0.id, label: $0.label) })
                feedback = ""
                return
            }
        }
        let call = ToolCall(toolID: call.toolID, params: params)
        switch registry.plan(call, now: Date()) {
        case .planned(let action):
            pending = action
            feedback = ""
        case .invalid(let message):
            pending = nil
            feedback = message
        case .needsChoice(let candidates):
            pending = nil
            guard allowChoice else { return }  // live preview stays quiet
            pendingChoice = PendingChoice(
                toolID: call.toolID, params: call.params, candidates: candidates)
            feedback = ""
        }
    }

    /// The user picked from the disambiguation list: re-plan with the exact id.
    func choose(_ candidate: ActionCandidate) {
        guard var choice = pendingChoice else { return }
        choice.params["chosen_id"] = .string(candidate.id)
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
        pending = nil
        pendingChoice = nil
        feedback = ""
        isPlanning = false
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
