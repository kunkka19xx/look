import AppKit
import Combine
import Foundation

/// The conversation half of the AI surface: the transcript, streaming answers,
/// clipboard text-ops, and persistence. Split from `ActionController`, which
/// keeps the propose/confirm/undo state machine - the two share a screen but
/// almost no state, and mixing them made one 1000-line object.
///
/// Observed directly by the view (`@ObservedObject var chat`), so its
/// `@Published` fields drive updates without republishing through a parent.
@MainActor
final class ChatSessionController: ObservableObject {
    static let shared = ChatSessionController()

    /// Completed actions, questions, and answers this session, shown as a stack
    /// in the AI panel. Ends on Esc, hide, or moving on to normal search.
    @Published private(set) var sessionItems: [ActionSessionItem] = []
    /// True while an answer is streaming, so the panel can offer Stop.
    @Published private(set) var isStreamingAnswer: Bool = false

    private var chatTask: Task<Void, Never>?
    /// Whether this submit's message is already in the transcript. Replaces a
    /// `userItemAppended` flag that every call path had to pass correctly.
    private var turns = TurnLedger()
    /// The answer item currently receiving tokens (settled when it ends).
    private var streamingItemID: UUID?
    private var conversationID = UUID()

    private init() {}

    var isActive: Bool { !sessionItems.isEmpty }

    // MARK: - Transcript

    func append(_ item: ActionSessionItem) {
        sessionItems.append(item)
    }

    /// A new user message is being handled. Called once per submit, before any
    /// routing, so whichever path ends up handling it can ask for the turn
    /// without knowing whether another path got there first.
    func startTurn() {
        turns.startTurn()
    }

    /// The user's message in the transcript, appended at most ONCE per submit.
    /// Later callers get the existing turn instead of a duplicate.
    @discardableResult
    func beginTurn(text: String, attachedPaths: [String] = []) -> UUID? {
        let item = ActionSessionItem(kind: .user, text: text, attachedPaths: attachedPaths)
        guard turns.shouldAppend(id: item.id) else { return turns.currentID }
        sessionItems.append(item)
        saveConversation()
        return item.id
    }

    func appendAction(_ summary: String) -> UUID {
        let item = ActionSessionItem(text: summary)
        sessionItems.append(item)
        saveConversation()
        return item.id
    }

    /// Marks an item's text (used by undo to annotate "· undone").
    func annotate(_ id: UUID, suffix: String) {
        guard let idx = sessionItems.firstIndex(where: { $0.id == id }) else { return }
        sessionItems[idx].text += suffix
        saveConversation()
    }

    /// Close the conversation: save it, drop in-flight work, clear the stack,
    /// and mint a fresh id.
    func endSession() {
        saveConversation()
        chatTask?.cancel()
        chatTask = nil
        finishStreaming()
        sessionItems.removeAll()
        conversationID = UUID()
        turns.reset()
    }

    /// Restore a stored conversation to continue it. The full transcript shows;
    /// the model context stays capped, so continuing an old conversation
    /// carries reasonable, bounded weight.
    func continueConversation(_ conversation: AIConversation) {
        endSession()
        conversationID = conversation.id
        sessionItems = conversation.items.map { stored in
            ActionSessionItem(
                kind: ActionSessionItem.Kind(rawValue: stored.kind) ?? .answer,
                text: stored.text,
                source: stored.source,
                attachedPaths: stored.attachedPaths ?? [])
        }
        turns.reset()
    }

    /// Upserts the live conversation into the capped store. Called whenever an
    /// item completes, so quitting mid-session loses nothing that finished.
    func saveConversation() {
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
                AIConversation.StoredItem(
                    kind: $0.kind.rawValue, text: $0.text, source: $0.source,
                    attachedPaths: $0.attachedPaths.isEmpty ? nil : $0.attachedPaths)
            }))
    }

    // MARK: - Streaming lifecycle

    /// Stop generating but KEEP the session and whatever text arrived (Esc ends
    /// the session instead). The partial answer is marked and archived.
    func stopGeneration() {
        guard isStreamingAnswer else { return }
        chatTask?.cancel()
        chatTask = nil
        if let id = streamingItemID, let idx = sessionItems.firstIndex(where: { $0.id == id }) {
            let partial = sessionItems[idx].text
            sessionItems[idx].text = partial == "…" || partial.isEmpty
                ? "(stopped)"
                : partial + "\n\n(stopped)"
        }
        finishStreaming()
        saveConversation()
    }

    func cancelStreaming() {
        chatTask?.cancel()
        chatTask = nil
        finishStreaming()
    }

    private func beginStreaming(_ id: UUID) {
        streamingItemID = id
        isStreamingAnswer = true
        setStreaming(true, for: id)
    }

    /// Ends the streaming state and settles the item (parses its markdown once).
    private func finishStreaming() {
        if let id = streamingItemID { setStreaming(false, for: id) }
        streamingItemID = nil
        isStreamingAnswer = false
    }

    private func setStreaming(_ streaming: Bool, for id: UUID) {
        guard let idx = sessionItems.firstIndex(where: { $0.id == id }) else { return }
        sessionItems[idx].isStreaming = streaming
    }

    // MARK: - Turns

    /// A chat turn: the question and a streaming answer stack as items, with the
    /// session (including performed actions) as conversation context. The turn
    /// itself is claimed through `beginTurn`, so calling this after the planner
    /// already showed the message adds an answer, not a second question.
    func askChat(_ query: String, attachments: MentionAttachments = MentionAttachments()) {
        chatTask?.cancel()
        // A leftover disambiguation list must not survive into an unrelated
        // turn: a later bare number would still fire the old (possibly
        // destructive) choice.
        ActionController.shared.clearPendingChoice()
        beginTurn(text: query, attachedPaths: attachments.paths)

        // Private context rides along only when the provider is on this machine
        // (or the user allowed remote context). Enforced, not assumed: see
        // AIQueryRouter.allowsPrivateContext.
        let settings = ThemeStore.shared.settings
        let allowsPrivate = AIQueryRouter.shared.allowsPrivateContext(settings.aiProvider)

        // Schedule-sounding questions get the calendar as context, so "what's my
        // next meeting" / "am I free friday" just answer.
        let asksSchedule = ScheduleContextProvider.mentionsSchedule(query)
        let listing = asksSchedule ? ScheduleContextProvider.listing(for: query) : nil
        if let listing { ActionController.shared.rememberListed(listing) }
        let scheduleContext: String? = {
            guard asksSchedule else { return nil }
            guard allowsPrivate else { return nil }
            return ScheduleContextProvider.chatContext(for: query, listing: listing)
        }()
        // A schedule question answered without the calendar would just look
        // uninformed; say why the context is missing.
        if asksSchedule, !allowsPrivate {
            append(ActionSessionItem(
                kind: .answer,
                text: Self.remoteContextBlockedNote("Your calendar"),
                source: "Look"))
        }

        // Long-term memory (facts the user asked to remember), injected on every
        // turn so the assistant "knows them" across conversations.
        let memoryContext = allowsPrivate ? MemoryStore.context() : ""

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
        // `@`-mentioned files. Same rule as the calendar: contents are private
        // context, so an off-machine provider is told why they are missing
        // rather than quietly answering about files it never saw.
        let fileContext = allowsPrivate ? attachments.contextBlock() : ""
        if !fileContext.isEmpty {
            messages.append([
                "role": "system",
                "content": "Files the user attached to this question:\n\(fileContext)",
            ])
        }
        if !attachments.isEmpty, !allowsPrivate {
            append(ActionSessionItem(
                kind: .answer,
                text: Self.remoteContextBlockedNote("Attached files"),
                source: "Look"))
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

        let placeholderID = appendPlaceholder(settings: settings)

        // Both paths now send the SAME messages: memory and schedule context
        // are system messages built above, so no provider needs them folded
        // into the user's question any more.
        let provider = settings.aiProvider

        if provider == .ollama {
            // Attached files make this a document read, not a chat reply.
            startOllamaStream(
                messages: messages, settings: settings, into: placeholderID,
                options: attachments.isEmpty
                    ? nil
                    : Self.ollamaOptions(
                        .document(promptCharacters: attachments.totalCharacters)))
            return
        }

        // Structured: the system prompt and the context blocks stay SYSTEM
        // messages instead of being pasted into the user's question, which is
        // what a provider outside the Ollama path used to receive.
        let structured = messages.map {
            AIMessage(AIMessage.Role(rawValue: $0["role"] ?? "user") ?? .user, $0["content"] ?? "")
        }
        let requested: AIGenerationOptions = attachments.isEmpty
            ? .chat
            : .document(promptCharacters: attachments.totalCharacters)
        let makeStream: @MainActor () -> AsyncThrowingStream<String, Error>? = {
            AIQueryRouter.shared.respond(
                messages: structured, options: requested, using: provider)
        }
        if let stream = makeStream() {
            chatTask = Task { [weak self] in await self?.consume(stream, into: placeholderID) }
            return
        }

        // Schedule questions don't actually need a model: answer with the
        // deterministic listing.
        if let listing {
            updateItem(
                placeholderID, text: "Your calendar \(listing.label):\n\(listing.summary)",
                source: "Calendar")
            saveConversation()
            return
        }

        // FoundationModels can report unavailable right after app launch until
        // first touched; the prewarm touches it, so retry briefly before
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

    /// A text op: transform the text from `source` with the given instruction,
    /// streaming the result into the panel (reusing chat). Returns the feedback
    /// the caller should show, or nil once the op is running.
    @discardableResult
    func runTextOp(label: String, instruction: String, source: TextOpSource) -> String? {
        let input: String
        let subject: String
        var truncatedFrom: String?

        switch source {
        case .ambiguous(let count):
            return "\(count) items picked. Pick one file, then try \"\(label.lowercased())\"."
        case .clipboard:
            let clip = (NSPasteboard.general.string(forType: .string) ?? "")
                .trimmingCharacters(in: .whitespacesAndNewlines)
            guard !clip.isEmpty else {
                return "Copy some text first, then try \"\(label.lowercased())\"."
            }
            input = clip
            subject = "Clipboard text"
        case .file(let path):
            let name = (path as NSString).lastPathComponent
            switch TextExtraction.extract(path: path) {
            case .success(let extracted):
                input = extracted.text
                subject = "File contents"
                if extracted.truncated { truncatedFrom = name }
            case .failure(.notText):
                return "\"\(name)\" is not a text file."
            case .failure(.empty):
                return "\"\(name)\" is empty."
            case .failure(.unreadable):
                return "Could not read \"\(name)\"."
            }
        }

        // Clipboard and file contents are the most sensitive context of all:
        // never hand either to an off-machine provider without permission.
        guard AIQueryRouter.shared.allowsPrivateContext(ThemeStore.shared.settings.aiProvider)
        else {
            append(ActionSessionItem(
                kind: .answer,
                text: Self.remoteContextBlockedNote(subject),
                source: "Look"))
            saveConversation()
            return nil
        }
        chatTask?.cancel()
        beginTurn(
            text: Self.turnLabel(label, source, input: input),
            attachedPaths: {
                if case .file(let path) = source { return [path] }
                return []
            }())
        // A capped read is a partial answer, so say so instead of passing the
        // head of the file off as the whole of it.
        if let name = truncatedFrom {
            append(ActionSessionItem(
                kind: .answer,
                text: "\"\(name)\" is large, so this covers only its first "
                    + "\(TextExtraction.defaultCap / 1024)KB.",
                source: "Look"))
        }
        saveConversation()

        let settings = ThemeStore.shared.settings
        let placeholderID = appendPlaceholder(settings: settings)
        let messages: [[String: String]] = [
            ["role": "system", "content": instruction],
            ["role": "user", "content": input],
        ]

        if settings.aiProvider == .ollama {
            // The input IS a document here, so it gets the wider ceilings.
            startOllamaStream(
                messages: messages, settings: settings, into: placeholderID,
                options: Self.ollamaOptions(.document(promptCharacters: input.count)))
            return nil
        }

        // The instruction is an instruction, not part of the text being
        // transformed: a provider that understands roles now gets it as one.
        if let stream = AIQueryRouter.shared.respond(
            messages: [AIMessage(.system, instruction), AIMessage(.user, input)],
            options: .document(promptCharacters: input.count),
            using: settings.aiProvider)
        {
            chatTask = Task { [weak self] in await self?.consume(stream, into: placeholderID) }
            return nil
        }
        updateItem(placeholderID, text: chatFailureMessage())
        saveConversation()
        return nil
    }

    /// The user turn shows WHAT was transformed, not just the verb: a file
    /// contributes its path (its contents are the answer's subject, not the
    /// request), the clipboard contributes the text itself, since otherwise
    /// "Translate to vietnamese" gives a transcript with no way to tell which
    /// of five copies it acted on.
    private static func turnLabel(_ label: String, _ source: TextOpSource, input: String)
        -> String
    {
        switch source {
        // The file rides along as a capsule under the turn, so repeating its
        // path here would just say the same thing twice.
        case .file: label
        case .clipboard: input.isEmpty ? label : "\(label)\n\(input)"
        case .ambiguous: label
        }
    }

    /// The answer placeholder, labeled with whoever is about to produce it.
    private func appendPlaceholder(settings: ThemeSettings) -> UUID {
        let sourceLabel = settings.aiProvider == .ollama
            ? settings.ollamaModel
            : settings.aiProvider.title
        let placeholder = ActionSessionItem(
            kind: .answer, text: "…", source: sourceLabel, isStreaming: true)
        sessionItems.append(placeholder)
        return placeholder.id
    }

    /// Streamed through the Rust core (curl child + polling), the same transport
    /// linows will use and the same one the answer card rides.
    private func startOllamaStream(
        messages: [[String: String]], settings: ThemeSettings, into id: UUID,
        options: String? = nil
    ) {
        if let data = try? JSONSerialization.data(withJSONObject: messages),
           let json = String(data: data, encoding: .utf8) {
            let session = EngineBridge.shared.aiChatStart(
                host: settings.ollamaEndpoint, model: settings.ollamaModel, messagesJSON: json,
                optionsJSON: options ?? Self.ollamaOptions(.chat))
            if session != 0 {
                chatTask = Task { [weak self] in
                    await self?.consumeChatSession(session, into: id)
                }
                return
            }
        }
        updateItem(id, text: chatFailureMessage())
        saveConversation()
    }

    // MARK: - Consumption

    /// Polls a Rust-core chat session (~12x/sec) into the answer item, then
    /// saves. Cancellation aborts the curl child, stopping generation.
    private func consumeChatSession(_ session: UInt64, into id: UUID) async {
        beginStreaming(id)
        defer {
            if Task.isCancelled { EngineBridge.shared.aiChatCancel(session) }
            // stopGeneration() already settled the item; this covers the
            // normal end and any error exit.
            if streamingItemID == id { finishStreaming() }
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
                // Returning here (rather than waiting for `done`) matches the
                // other consumers and guarantees the Stop affordance clears
                // even if the core reported an error without a final done.
                let message = chatFailureMessage(detail: error)
                updateItem(
                    id,
                    text: snapshot.text.isEmpty ? message : snapshot.text + "\n\n" + message)
                saveConversation()
                return
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
        beginStreaming(id)
        defer { if streamingItemID == id { finishStreaming() } }
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

    /// Said once, in place, when personal data was withheld from a remote
    /// provider - so the gap is visible rather than a mysteriously worse answer.
    static func remoteContextBlockedNote(_ what: String) -> String {
        "\(what) isn't sent to a model outside this machine. Point the AI "
            + "provider at a local model, or allow it in Settings."
    }

    /// Ollama's dialect for a neutral request. The Ollama path talks to the
    /// Rust transport directly (for cancellation), so it translates here rather
    /// than through the provider - but through the SAME translation.
    private static func ollamaOptions(_ options: AIGenerationOptions) -> String {
        options.ollamaJSON(contextCeiling: AIQueryRouter.shared.contextTokens(of: .ollama))
    }

    private static let chatInstructions = """
        You are Look's built-in assistant on macOS. Be concise and helpful. \
        Plain text; short code snippets are fine when asked. Calendar and \
        reminder changes (adding, moving, cancelling, completing) happen \
        outside this chat: the user types the request and confirms it on a \
        preview bar. If asked to make one, tell them to type it as an \
        instruction (e.g. "move my dentist to friday") and confirm it - do not \
        claim you cannot do it.
        """
}
