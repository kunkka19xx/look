import SwiftUI

/// /emoji results grid. Selection is the first cell, which Enter copies and
/// Shift+Enter inserts; clicking a cell copies it.
struct EmojiView: View {
    let query: String
    let onPick: (EmojiHitDTO) -> Void

    private static let limit = 60
    private let columns = Array(repeating: GridItem(.flexible(), spacing: 4), count: 8)

    var body: some View {
        let hits = EmojiView.hits(for: query)
        ScrollView {
            if hits.isEmpty {
                Text(query.trimmingCharacters(in: .whitespaces).isEmpty
                    ? "Type to search emoji" : "No emoji found")
                    .foregroundStyle(.secondary)
                    .padding(10)
            } else {
                LazyVGrid(columns: columns, spacing: 4) {
                    ForEach(Array(hits.enumerated()), id: \.offset) { index, hit in
                        Text(hit.glyph)
                            .font(.system(size: 24))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 4)
                            .background(
                                RoundedRectangle(cornerRadius: 6)
                                    .fill(index == 0 ? Color.accentColor.opacity(0.25) : .clear)
                            )
                            .help(hit.name)
                            .onTapGesture { onPick(hit) }
                    }
                }
                .padding(8)
            }
        }
    }

    /// Recents when the query is empty, ranked matches otherwise.
    static func hits(for query: String) -> [EmojiHitDTO] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            return EmojiRecents.decode(UserDefaults.standard.data(forKey: recentsKey))
                .map { EmojiHitDTO(glyph: $0.glyph, name: $0.name) }
        }
        return EngineBridge.shared.emojiSearch(query: trimmed, limit: limit)
    }

    static let recentsKey = "look.emoji.recents"

    static func remember(_ hit: EmojiHitDTO) {
        let current = EmojiRecents.decode(UserDefaults.standard.data(forKey: recentsKey))
        let next = EmojiRecents.pushed(current, .init(glyph: hit.glyph, name: hit.name))
        UserDefaults.standard.set(EmojiRecents.encode(next), forKey: recentsKey)
    }
}
