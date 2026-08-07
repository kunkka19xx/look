import Foundation

/// One saved AI conversation: actions, questions, and answers in order.
struct AIConversation: Codable, Identifiable {
    struct StoredItem: Codable {
        var kind: String
        var text: String
        var source: String?
    }

    var id: UUID
    var title: String
    var updatedAt: Date
    var items: [StoredItem]
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
}
