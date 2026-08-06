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
}

@MainActor
final class ActionController: ObservableObject {
    static let shared = ActionController()

    @Published private(set) var pending: PlannedAction?
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

    /// Leave the AI session entirely: sweep anything not yet archived (e.g. a
    /// partial answer cut off by Esc), then drop pending work, results, and the
    /// item stack. Fired only by Esc with nothing pending.
    func endSession() {
        for item in sessionItems { archive(item) }
        cancel()
        sessionItems.removeAll()
        lastReceipt = nil
        undoableItemID = nil
        sessionID = UUID().uuidString
        archivedIDs.removeAll()
    }

    private var sessionID = UUID().uuidString
    private var archivedIDs: Set<UUID> = []

    /// Incremental transcript: one JSONL line per item, appended the moment the
    /// item completes, so nothing is lost if the app quits mid-session. File:
    /// `~/Library/Application Support/Look/ai-sessions.jsonl`. Lines share a
    /// `session` id so a conversation can be reassembled.
    private func archive(_ item: ActionSessionItem) {
        guard !archivedIDs.contains(item.id) else { return }
        // Don't archive an answer that never got content (placeholder only).
        if item.kind == .answer,
           item.text == "…" || item.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return
        }
        archivedIDs.insert(item.id)
        Self.appendArchiveRecord(kind: item.kind.rawValue, text: item.text, session: sessionID)
    }

    private static func appendArchiveRecord(kind: String, text: String, session: String) {
        let record: [String: Any] = [
            "ts": ISO8601DateFormatter().string(from: Date()),
            "session": session,
            "kind": kind,
            "text": text,
        ]
        guard
            let data = try? JSONSerialization.data(withJSONObject: record),
            let line = String(data: data, encoding: .utf8),
            let dir = FileManager.default.urls(
                for: .applicationSupportDirectory, in: .userDomainMask
            ).first?.appendingPathComponent("Look", isDirectory: true)
        else { return }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let file = dir.appendingPathComponent("ai-sessions.jsonl")
        let payload = Data((line + "\n").utf8)
        if let handle = try? FileHandle(forWritingTo: file) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: payload)
        } else {
            try? payload.write(to: file)
        }
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

        // Explicit `@` form: instant, exact, no model.
        if let call = ExplicitActionParser.parse(">" + query),
           case .planned(let action) = registry.plan(call, now: Date()) {
            isPlanning = false
            pending = action
            feedback = ""
            return
        }

        // Natural language: never show a rough/wrong title. Instead show the
        // working indicator IMMEDIATELY (no dead air) and normalize in the
        // background; the clean preview replaces the indicator when ready.
        guard planner.isAvailable else {
            pending = nil
            isPlanning = false
            feedback = "Connect a model, or use: >add <title> @ <time>"
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
                self.propose(call)
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

        if let call = ExplicitActionParser.parse(">" + query),
           case .planned(let action) = registry.plan(call, now: Date()) {
            planTask?.cancel()
            isPlanning = false
            pending = action
            feedback = ""
            return
        }

        guard planner.isAvailable else {
            feedback = "Couldn't parse. Try: >add <title> @ <time>"
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
        archive(userItem)

        var messages: [[String: String]] = [
            ["role": "system", "content": Self.chatInstructions]
        ]
        // Schedule-sounding questions get the calendar as context, so "what's my
        // next meeting" / "am I free friday" just answer. Injected after the
        // static prompt so the prompt-cache prefix stays effective; the data
        // only ever goes to the local model.
        if Self.mentionsSchedule(query),
           let summary = EventKitService.shared.upcomingEventsSummary() {
            let df = DateFormatter()
            df.dateFormat = "EEEE, MMMM d yyyy, HH:mm"
            messages.append([
                "role": "system",
                "content": "Now: \(df.string(from: Date())). "
                    + "The user's calendar for the next 7 days:\n\(summary)",
            ])
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

        let placeholder = ActionSessionItem(kind: .answer, text: "…")
        sessionItems.append(placeholder)
        let placeholderID = placeholder.id
        let settings = ThemeStore.shared.settings

        chatTask = Task { [weak self] in
            guard let self else { return }
            let stream = OllamaProvider.chatStream(
                host: settings.ollamaHost, model: settings.ollamaModel, messages: messages)
            do {
                for try await partial in stream {
                    if Task.isCancelled { return }
                    self.updateItem(placeholderID, text: partial)
                }
            } catch {
                if !Task.isCancelled {
                    self.updateItem(placeholderID, text: "Answer failed. Is Ollama running?")
                }
            }
            // Stream done (or failed with a message): archive the final answer.
            if !Task.isCancelled,
               let item = self.sessionItems.first(where: { $0.id == placeholderID }) {
                self.archive(item)
            }
        }
    }

    private func updateItem(_ id: UUID, text: String) {
        guard let idx = sessionItems.firstIndex(where: { $0.id == id }) else { return }
        sessionItems[idx].text = text
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

    func propose(_ call: ToolCall) {
        switch registry.plan(call, now: Date()) {
        case .planned(let action):
            pending = action
            feedback = ""
        case .invalid(let message):
            pending = nil
            feedback = message
        case .needsChoice:
            pending = nil
            feedback = "Multiple matches. Be more specific."
        }
    }

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
                let item = ActionSessionItem(text: receipt.summary)
                sessionItems.append(item)
                undoableItemID = item.id
                archive(item)
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
        feedback = ""
        isPlanning = false
    }

    func undoLast() {
        guard let receipt = lastReceipt else { return }
        do {
            try receipt.undo()
            if let idx = sessionItems.firstIndex(where: { $0.id == undoableItemID }) {
                sessionItems[idx].text += "  ·  undone"
                Self.appendArchiveRecord(
                    kind: "undo", text: sessionItems[idx].text, session: sessionID)
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
