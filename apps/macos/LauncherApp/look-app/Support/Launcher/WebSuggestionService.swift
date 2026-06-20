import Foundation

/// Search autocomplete suggestions. Uses DuckDuckGo's autocomplete endpoint,
/// which returns clean UTF-8 `[query, [suggestions]]` — unlike Google's
/// `complete/search`, which serves ISO-8859-1 and breaks on non-ASCII input
/// (e.g. Vietnamese). Pressing Enter still runs a Google *search*; only the
/// completions come from here. Best-effort: returns `[]` on any failure.
enum WebSuggestionService {
    nonisolated static func suggestions(for query: String, limit: Int) async -> [String] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count >= 2 else { return [] }

        guard let array = await WebJSON.array(WebJSON.url(
            "https://duckduckgo.com/ac/",
            [
                URLQueryItem(name: "q", value: trimmed),
                URLQueryItem(name: "type", value: "list"),
            ]
        )), array.count >= 2, let list = array[1] as? [String] else { return [] }

        // Drop the echo of the query itself and any duplicates; cap the count.
        let lowerQuery = trimmed.lowercased()
        var seen = Set<String>()
        var result: [String] = []
        for suggestion in list {
            let text = suggestion.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty,
                  text.lowercased() != lowerQuery,
                  seen.insert(text.lowercased()).inserted
            else { continue }
            result.append(text)
            if result.count >= limit { break }
        }
        return result
    }
}
