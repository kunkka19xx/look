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

/// Capped local store of AI conversations, one human-readable JSON file at
/// `~/Library/Application Support/Look/ai-conversations.json`, newest first.
/// Upserts happen incrementally as items complete, so quitting mid-session
/// loses nothing that finished. Bounds keep it small: 20 conversations, the
/// last 60 items each.
nonisolated enum ConversationStore {
    static let conversationLimit = 20
    static let itemLimit = 60

    private static var fileURL: URL? {
        guard
            let dir = FileManager.default.urls(
                for: .applicationSupportDirectory, in: .userDomainMask
            ).first?.appendingPathComponent("Look", isDirectory: true)
        else { return nil }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("ai-conversations.json")
    }

    static func load() -> [AIConversation] {
        guard let url = fileURL, let data = try? Data(contentsOf: url) else { return [] }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? decoder.decode([AIConversation].self, from: data)) ?? []
    }

    static func upsert(_ conversation: AIConversation) {
        var convo = conversation
        if convo.items.count > itemLimit {
            convo.items = Array(convo.items.suffix(itemLimit))
        }
        var list = load().filter { $0.id != convo.id }
        list.insert(convo, at: 0)
        list.sort { $0.updatedAt > $1.updatedAt }
        if list.count > conversationLimit {
            list = Array(list.prefix(conversationLimit))
        }
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        guard let url = fileURL, let data = try? encoder.encode(list) else { return }
        try? data.write(to: url)
    }
}
