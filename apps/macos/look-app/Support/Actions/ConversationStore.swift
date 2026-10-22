import Foundation

/// One saved AI conversation: actions, questions, and answers in order.
struct AIConversation: Codable, Identifiable {
    struct StoredItem: Codable {
        var kind: String
        var text: String
        var source: String?
        /// Optional so conversations written before `@`-mentions still decode.
        var attachedPaths: [String]?
    }

    var id: UUID
    var title: String
    var updatedAt: Date
    var items: [StoredItem]

    /// How much of the first message becomes the title.
    static let titleLimit = 48

    /// One line, whitespace collapsed. A title is drawn in a list row and in the
    /// delete banner, and the message it comes from can carry newlines (pasted
    /// text, or Shift+Enter in the composer) - which render as a stack of short
    /// rows rather than one line. Applied when the title is MADE and again when
    /// it is DRAWN, so conversations stored before this stay tidy too.
    static func singleLine(_ text: String, limit: Int = titleLimit) -> String {
        String(text.split(whereSeparator: \.isWhitespace).joined(separator: " ").prefix(limit))
    }

    /// The title as one line, for any surface that draws it.
    func displayTitle(limit: Int = AIConversation.titleLimit) -> String {
        AIConversation.singleLine(title, limit: limit)
    }
}

/// Thin shell over the Rust-core conversation store (core/ai), which owns the
/// caps (20 conversations x 60 items) and the file format. The shell supplies
/// the platform path: `~/Library/Application Support/Look/ai-conversations.json`.
@MainActor
enum ConversationStore {
    private static var filePath: String? {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?
            .appendingPathComponent("Look", isDirectory: true)
            .appendingPathComponent("ai-conversations.json")
            .path
    }

    static func load() -> [AIConversation] {
        guard
            let path = filePath,
            let data = EngineBridge.shared.aiConversationsJSON(path: path)
        else { return [] }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? decoder.decode([AIConversation].self, from: data)) ?? []
    }

    static func upsert(_ conversation: AIConversation) {
        guard let path = filePath else { return }
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        guard
            let data = try? encoder.encode(conversation),
            let json = String(data: data, encoding: .utf8)
        else { return }
        _ = EngineBridge.shared.aiConversationUpsert(path: path, json: json)
    }

    @discardableResult
    static func delete(id: UUID) -> Bool {
        guard let path = filePath else { return false }
        return EngineBridge.shared.aiConversationDelete(path: path, id: id.uuidString)
    }
}
