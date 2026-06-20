import Foundation

/// An instant, web-sourced answer (à la Spotlight's knowledge card). Returned
/// finished — no streaming — because it's a cached lookup, not generation.
struct WebAnswer: Sendable {
    let text: String
    let source: String
    let url: URL?
    let imageURL: URL?
}

/// Fetches a quick factual answer from free web sources before we fall back to
/// the (slower) on-device model. Coverage is deliberately narrow: encyclopedic
/// facts and definitions, which is exactly what these APIs do well.
enum WebAnswerService {
    /// DuckDuckGo instant answer, or nil. Source is labelled "DuckDuckGo" so it
    /// reads distinctly from a direct Wikipedia hit when both are shown.
    nonisolated static func duckDuckGoAnswer(query: String) async -> WebAnswer? {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let object = await WebJSON.object(WebJSON.url("https://api.duckduckgo.com/", [
                  URLQueryItem(name: "q", value: trimmed),
                  URLQueryItem(name: "format", value: "json"),
                  URLQueryItem(name: "no_html", value: "1"),
                  URLQueryItem(name: "skip_disambig", value: "1"),
              ]))
        else { return nil }

        let answer = (object["Answer"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let abstract = (object["AbstractText"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let text = answer.isEmpty ? abstract : answer
        guard !text.isEmpty else { return nil }

        let pageURL = (object["AbstractURL"] as? String).flatMap(URL.init(string:))
        let imageURL = imageURLFromDuckDuckGo(object["Image"] as? String)
        return WebAnswer(text: text, source: "DuckDuckGo", url: pageURL, imageURL: imageURL)
    }

    /// Wikipedia summary for an already-chosen search term (the caller decides
    /// whether a query warrants a Wikipedia lookup and what to search for).
    nonisolated static func wikipediaAnswer(searchTerm: String) async -> WebAnswer? {
        let trimmed = searchTerm.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        // Resolve free text to a real article title first.
        guard let parsed = await WebJSON.array(WebJSON.url("https://en.wikipedia.org/w/api.php", [
                  URLQueryItem(name: "action", value: "opensearch"),
                  URLQueryItem(name: "format", value: "json"),
                  URLQueryItem(name: "limit", value: "1"),
                  URLQueryItem(name: "search", value: trimmed),
              ])),
              parsed.count >= 4,
              let title = (parsed[1] as? [String])?.first
        else { return nil }

        let pageURL = (parsed[3] as? [String])?.first.flatMap(URL.init(string:))
        let encoded = title.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? title
        guard let object = await WebJSON.object(URL(string: "https://en.wikipedia.org/api/rest_v1/page/summary/\(encoded)")),
              let extract = (object["extract"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
              !extract.isEmpty
        else { return nil }

        // Disambiguation pages ("Vim may refer to:") aren't real answers.
        if (object["type"] as? String) == "disambiguation" { return nil }

        let imageURL = ((object["thumbnail"] as? [String: Any])?["source"] as? String)
            .flatMap(URL.init(string:))
        return WebAnswer(text: extract, source: "Wikipedia", url: pageURL, imageURL: imageURL)
    }

    /// DuckDuckGo's `Image` is often a site-relative path ("/i/abc.png").
    nonisolated private static func imageURLFromDuckDuckGo(_ raw: String?) -> URL? {
        guard let raw, !raw.isEmpty else { return nil }
        if raw.hasPrefix("http") { return URL(string: raw) }
        return URL(string: "https://duckduckgo.com\(raw)")
    }

    // MARK: - Heuristics

    /// Extracts the entity from a definitional query ("what is vim" -> "vim"),
    /// or returns `nil` when the query isn't asking for a definition.
    nonisolated static func definitionalEntity(in query: String) -> String? {
        let lower = query.lowercased()
        let prefixes = [
            "what is ", "what are ", "what was ", "what's ", "whats ",
            "who is ", "who was ", "who are ", "who's ",
            "define ", "definition of ", "meaning of ", "tell me about ",
        ]
        for prefix in prefixes where lower.hasPrefix(prefix) {
            let entity = String(query.dropFirst(prefix.count))
                .trimmingCharacters(in: CharacterSet(charactersIn: " ?.!"))
            return entity.isEmpty ? nil : entity
        }
        return nil
    }
}
