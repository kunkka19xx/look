import Foundation

/// One message of a request, with its role intact. Before this, every provider
/// that was not Ollama received `"\(instruction)\n\n\(text)"` - the roles were
/// flattened away at the call site, so on-device and (later) cloud providers
/// could not be given a system prompt at all.
nonisolated struct AIMessage: Equatable, Sendable {
    enum Role: String, Sendable { case system, user, assistant }

    let role: Role
    let content: String

    init(_ role: Role, _ content: String) {
        self.role = role
        self.content = content
    }

    /// For providers with no notion of roles. The last resort, not the default.
    static func flattened(_ messages: [AIMessage]) -> String {
        messages
            .filter { !$0.content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
            .map(\.content)
            .joined(separator: "\n\n")
    }

    /// The system half, which several providers take separately from the turn.
    static func instructions(_ messages: [AIMessage]) -> String {
        messages.filter { $0.role == .system }.map(\.content).joined(separator: "\n\n")
    }

    /// Everything that is not a system message, as one prompt. A provider that
    /// takes a single string (Apple Intelligence) still has to be able to tell
    /// the user's turns from its own: an unlabeled blob of a restored
    /// conversation reads as one enormous question. A lone user message is left
    /// bare, since labelling a single question would just be noise.
    static func conversation(_ messages: [AIMessage]) -> String {
        // Empty turns are dropped BEFORE the count, or a lone question sitting
        // beside a blank turn counts as two and gets labelled as history.
        let turns = messages.filter {
            $0.role != .system
                && !$0.content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        guard turns.count > 1 else { return flattened(turns) }
        return turns
            .map { turn in
                switch turn.role {
                case .assistant: "Assistant: \(turn.content)"
                case .user, .system: "User: \(turn.content)"
                }
            }
            .joined(separator: "\n\n")
    }
}

/// What a request NEEDS, in terms every provider can answer in its own
/// vocabulary. Deliberately not Ollama's: `num_ctx` is meaningless to a cloud
/// model whose context is fixed by the model chosen, and `max_tokens` is
/// meaningless to Apple Intelligence, which caps its own response.
nonisolated struct AIGenerationOptions: Equatable, Sendable {
    var maxOutputTokens: Int
    /// Roughly how large the prompt is. A HINT, not a limit: providers that
    /// must reserve their window up front (Ollama) size it from this, and
    /// providers with a fixed window ignore it.
    var expectedPromptTokens: Int
    var temperature: Double
    var timeoutSeconds: Int

    init(
        maxOutputTokens: Int = 512,
        expectedPromptTokens: Int = 0,
        temperature: Double = 0,
        timeoutSeconds: Int = 300
    ) {
        self.maxOutputTokens = maxOutputTokens
        self.expectedPromptTokens = expectedPromptTokens
        self.temperature = temperature
        self.timeoutSeconds = timeoutSeconds
    }

    /// A launcher answer card: a few sentences, gives up fast.
    static let answerCard = AIGenerationOptions(
        maxOutputTokens: 220, temperature: 0.4, timeoutSeconds: 45)

    /// A session chat reply.
    static let chat = AIGenerationOptions(maxOutputTokens: 512)

    /// Reading a document: room to answer at length, and a prompt estimate so a
    /// provider that reserves context can hold the whole file.
    static func document(promptCharacters: Int) -> AIGenerationOptions {
        AIGenerationOptions(
            maxOutputTokens: 2048,
            expectedPromptTokens: promptCharacters / charactersPerToken)
    }

    /// Rough English prose ratio. Only ever used to size a window or warn, never
    /// to trim: an estimate that silently dropped text would be worse than the
    /// truncation it guards against.
    static let charactersPerToken = 4

    /// Ollama's dialect. The ONE place this translation happens, so adding a
    /// provider means writing its own translation, not editing shared code.
    /// `num_ctx` is included only when the prompt needs more than the daemon's
    /// 4096 default, since a bigger KV cache costs memory on every request.
    func ollamaJSON(contextCeiling: Int) -> String {
        var fields: [String] = [
            "\"num_predict\":\(maxOutputTokens)",
            "\"temperature\":\(temperature)",
            "\"timeout_secs\":\(timeoutSeconds)",
        ]
        let needed = expectedPromptTokens + maxOutputTokens + 1024
        if needed > 4096 {
            var context = 4096
            while context < needed, context < contextCeiling { context *= 2 }
            fields.append("\"num_ctx\":\(min(context, contextCeiling))")
        }
        return "{\(fields.joined(separator: ","))}"
    }
}
