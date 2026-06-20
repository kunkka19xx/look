import Foundation

/// Tiny shared JSON-over-HTTP client for the AI answer sources. Centralises the
/// one ephemeral `URLSession` and the fetch-then-decode boilerplate so each
/// source just builds a URL and reads fields. Returns `nil` on any failure
/// (transport error, non-2xx, or non-JSON) — sources are best-effort.
enum WebJSON {
    nonisolated private static let session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.waitsForConnectivity = false
        // Some APIs reject a missing User-Agent; harmless for the rest.
        config.httpAdditionalHeaders = ["User-Agent": "Look-Launcher"]
        return URLSession(configuration: config)
    }()

    /// Builds a URL from a base string + query items.
    nonisolated static func url(_ base: String, _ query: [URLQueryItem]) -> URL? {
        var components = URLComponents(string: base)
        components?.queryItems = query
        return components?.url
    }

    /// GETs `url` and decodes a top-level JSON object, or `nil`.
    nonisolated static func object(_ url: URL?, timeout: TimeInterval = 4) async -> [String: Any]? {
        await json(url, timeout: timeout) as? [String: Any]
    }

    /// GETs `url` and decodes a top-level JSON array, or `nil`.
    nonisolated static func array(_ url: URL?, timeout: TimeInterval = 4) async -> [Any]? {
        await json(url, timeout: timeout) as? [Any]
    }

    nonisolated private static func json(_ url: URL?, timeout: TimeInterval) async -> Any? {
        guard let url else { return nil }
        var request = URLRequest(url: url)
        request.timeoutInterval = timeout
        guard let (data, response) = try? await session.data(for: request) else { return nil }
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) { return nil }
        return try? JSONSerialization.jsonObject(with: data)
    }
}
