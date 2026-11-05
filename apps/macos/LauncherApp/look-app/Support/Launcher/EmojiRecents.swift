import Foundation

/// Pure /emoji recents logic, free of AppKit so it can be unit-tested in the
/// LauncherLogic package.
enum EmojiRecents {
    static let cap = 20

    struct Entry: Codable, Equatable {
        let glyph: String
        let name: String
    }

    static func pushed(_ list: [Entry], _ entry: Entry) -> [Entry] {
        Array(([entry] + list.filter { $0.glyph != entry.glyph }).prefix(cap))
    }

    static func encode(_ list: [Entry]) -> Data {
        (try? JSONEncoder().encode(list)) ?? Data()
    }

    static func decode(_ data: Data?) -> [Entry] {
        guard let data, let list = try? JSONDecoder().decode([Entry].self, from: data) else {
            return []
        }
        return Array(list.prefix(cap))
    }
}
