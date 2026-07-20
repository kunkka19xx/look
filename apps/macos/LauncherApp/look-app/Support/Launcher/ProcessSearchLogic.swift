import Foundation

enum ProcessSearchLogic {
    static func score(query: String, name: String, pid: Int32) -> Int? {
        let needle = normalize(query).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else { return 0 }

        let pidText = String(pid)
        if pidText == needle { return 10_000 }
        if pidText.hasPrefix(needle) { return 9_000 - (pidText.count - needle.count) }
        if pidText.contains(needle) { return 8_000 - (pidText.count - needle.count) }

        return textScore(needle: needle, haystack: normalize(name))
    }

    private static func textScore(needle: String, haystack: String) -> Int? {
        if haystack == needle { return 7_000 }
        if haystack.hasPrefix(needle) { return 6_000 - (haystack.count - needle.count) }
        if let range = haystack.range(of: needle) {
            return 5_000 - haystack.distance(from: haystack.startIndex, to: range.lowerBound)
        }

        var needleIndex = needle.startIndex
        var previousOffset: Int?
        var firstOffset: Int?
        var totalGap = 0
        var consecutive = 0

        for (offset, character) in haystack.enumerated() where character == needle[needleIndex] {
            firstOffset = firstOffset ?? offset
            if let previousOffset {
                let gap = offset - previousOffset - 1
                totalGap += gap
                if gap == 0 { consecutive += 1 }
            }
            previousOffset = offset
            needleIndex = needle.index(after: needleIndex)
            if needleIndex == needle.endIndex {
                return 3_000
                    + needle.count * 20
                    + consecutive * 10
                    - totalGap
                    - (firstOffset ?? 0)
            }
        }

        return nil
    }

    private static func normalize(_ text: String) -> String {
        text.folding(
            options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive],
            locale: nil
        )
        .replacingOccurrences(of: "đ", with: "d")
        .replacingOccurrences(of: "Đ", with: "d")
    }
}
