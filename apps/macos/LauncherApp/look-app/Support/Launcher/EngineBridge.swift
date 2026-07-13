import Foundation

@_silgen_name("look_search_json")
nonisolated
private func look_search_json(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_search_json_compact")
nonisolated
private func look_search_json_compact(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_record_usage_json")
nonisolated
private func look_record_usage_json(_ candidateID: UnsafePointer<CChar>?, _ action: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_free_cstring")
nonisolated
private func look_free_cstring(_ ptr: UnsafeMutablePointer<CChar>?)

@_silgen_name("look_reload_config")
nonisolated
private func look_reload_config() -> Bool

@_silgen_name("look_request_index_refresh")
nonisolated
private func look_request_index_refresh() -> Bool

@_silgen_name("look_translate_json")
nonisolated
private func look_translate_json(_ text: UnsafePointer<CChar>?, _ targetLang: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_instant_answer_json")
nonisolated
private func look_instant_answer_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_instant_has_match")
nonisolated
private func look_instant_has_match(_ query: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_web_suggestions_json")
nonisolated
private func look_web_suggestions_json(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_duckduckgo_answer_json")
nonisolated
private func look_duckduckgo_answer_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_wikipedia_answer_json")
nonisolated
private func look_wikipedia_answer_json(_ searchTerm: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_classify_url_json")
nonisolated
private func look_classify_url_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_record_url_hit")
nonisolated
private func look_record_url_hit(_ url: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_recent_urls_json")
nonisolated
private func look_recent_urls_json(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_qactions_json")
nonisolated
private func look_qactions_json(_ resultID: UnsafePointer<CChar>?, _ kind: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_definitional_entity_json")
nonisolated
private func look_definitional_entity_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_todo_list_json")
nonisolated
private func look_todo_list_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_todo_save_json")
nonisolated
private func look_todo_save_json(_ json: UnsafePointer<CChar>?) -> Bool

final class EngineBridge: @unchecked Sendable {
    static let shared = EngineBridge()

    private init() {}

    nonisolated func search(query: String, limit: Int = 40) -> [LauncherResult] {
        let ptr = query.withCString { cstr in
            look_search_json_compact(cstr, UInt32(limit))
        }

        guard let ptr else {
            return fallbackResults()
        }

        defer {
            look_free_cstring(ptr)
        }

        let raw = String(cString: ptr)
        guard let data = raw.data(using: .utf8) else {
            return fallbackResults()
        }

        if let compactPayload = try? JSONDecoder().decode(CompactSearchPayload.self, from: data) {
            if compactPayload.error != nil {
                return fallbackResults()
            }
            return compactPayload.results.map { item in
                LauncherResult(
                    id: item.id,
                    kind: LauncherResultKind(rawValue: item.kind) ?? .app,
                    title: item.title,
                    subtitle: item.subtitle,
                    path: item.path,
                    score: item.score
                )
            }
        }

        // Compatibility fallback for older JSON payload shape.
        guard let fullPayload = try? JSONDecoder().decode(SearchPayload.self, from: data),
            fullPayload.error == nil
        else {
            return fallbackResults()
        }

        return fullPayload.results.map { item in
            LauncherResult(
                id: item.id,
                kind: LauncherResultKind(rawValue: item.kind) ?? .app,
                title: item.title,
                subtitle: item.subtitle,
                path: item.path,
                score: item.score
            )
        }
    }

    nonisolated func recordUsage(candidateID: String, action: String) -> BridgeError? {
        let ptr = candidateID.withCString { idCstr in
            action.withCString { actionCstr in
                look_record_usage_json(idCstr, actionCstr)
            }
        }

        guard let ptr else {
            return BridgeError(code: "ffi_null_response", message: "Usage tracking is temporarily unavailable")
        }

        defer {
            look_free_cstring(ptr)
        }

        let raw = String(cString: ptr)
        guard let data = raw.data(using: .utf8),
            let payload = try? JSONDecoder().decode(UsageRecordPayload.self, from: data)
        else {
            return BridgeError(code: "decode_failed", message: "Usage tracking response could not be decoded")
        }

        return payload.error
    }

    nonisolated func reloadConfig() -> Bool {
        look_reload_config()
    }

    /// Loads the full /todo task set from the shared core backend.
    nonisolated func todoList() -> [TodoBackendTask] {
        guard let ptr = look_todo_list_json() else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([TodoBackendTask].self, from: data)) ?? []
    }

    /// Persists the full /todo task set to the shared core backend
    /// (lossless replace). Returns true on success.
    @discardableResult
    nonisolated func todoSave(_ tasks: [TodoBackendTask]) -> Bool {
        guard let data = try? JSONEncoder().encode(tasks),
            let json = String(data: data, encoding: .utf8)
        else { return false }
        return json.withCString { look_todo_save_json($0) }
    }

    @discardableResult
    nonisolated func requestIndexRefresh() -> Bool {
        look_request_index_refresh()
    }

    nonisolated func translate(text: String, targetLang: String = "en") -> TranslationResult? {
        let result = text.withCString { textCstr in
            targetLang.withCString { langCstr in
                look_translate_json(textCstr, langCstr)
            }
        }

        guard let result else {
            return nil
        }

        defer {
            look_free_cstring(result)
        }

        let raw = String(cString: result)
        guard let data = raw.data(using: .utf8) else {
            return nil
        }

        return try? JSONDecoder().decode(TranslationResult.self, from: data)
    }

    /// Network-free gate: whether `query` matches a shared instant-answer
    /// provider (currency/weather/crypto). Cheap - safe to call while typing.
    nonisolated func instantAnswerMatches(_ query: String) -> Bool {
        query.withCString { look_instant_has_match($0) }
    }

    /// Resolves a shared instant answer (currency/weather/crypto) for `query`,
    /// or nil when nothing matches / the lookup fails. Blocking - call off the
    /// main thread (it performs network I/O in the Rust core).
    nonisolated func instantAnswer(query: String) -> WebAnswer? {
        decodeWebAnswer(query.withCString { look_instant_answer_json($0) })
    }

    /// DuckDuckGo instant answer for `query`, or nil. Blocking - call off-thread.
    nonisolated func duckDuckGoAnswer(query: String) -> WebAnswer? {
        decodeWebAnswer(query.withCString { look_duckduckgo_answer_json($0) })
    }

    /// Wikipedia summary for an already-chosen `searchTerm`, or nil. Blocking -
    /// call off-thread.
    nonisolated func wikipediaAnswer(searchTerm: String) -> WebAnswer? {
        decodeWebAnswer(searchTerm.withCString { look_wikipedia_answer_json($0) })
    }

    /// Up to `limit` search autocomplete suggestions for `query`. Blocking -
    /// call off-thread.
    nonisolated func webSuggestions(query: String, limit: Int) -> [String] {
        let ptr = query.withCString { look_web_suggestions_json($0, UInt32(limit)) }
        guard let ptr else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8),
            let list = try? JSONDecoder().decode([String].self, from: data)
        else { return [] }
        return list
    }

    /// Classifies `query` as a URL, or nil to leave it as a search term.
    /// Network-free; shares the Rust core's tier rules and TLD list with linows.
    nonisolated func classifyURL(query: String) -> URLMatch? {
        let ptr = query.withCString { look_classify_url_json($0) }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        let raw = String(cString: ptr)
        guard raw != "null", let data = raw.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(URLMatch.self, from: data)
    }

    /// Records that `url` was opened through the launcher, for later re-open
    /// suggestions. Fire-and-forget; opens the shared look.db (own connection).
    @discardableResult
    nonisolated func recordURLHit(url: String) -> Bool {
        url.withCString { look_record_url_hit($0) }
    }

    /// Up to `limit` previously-opened URLs matching `query`, most-recent first.
    /// Opens the shared look.db - call off the main thread.
    nonisolated func recentURLs(query: String, limit: Int) -> [URLHistoryEntry] {
        let ptr = query.withCString { look_recent_urls_json($0, UInt32(limit)) }
        guard let ptr else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return [] }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return (try? decoder.decode([URLHistoryEntry].self, from: data)) ?? []
    }

    /// Quick Action descriptors for a result, from the shared `look_qactions`
    /// catalog (see docs/writing-controls.md). Empty when the result has none.
    /// Pure catalog lookup - cheap, safe to call while typing.
    nonisolated func quickActions(forResultID resultID: String, kind: String) -> [QuickActionDescriptor] {
        let ptr = resultID.withCString { idCstr in
            kind.withCString { kindCstr in
                look_qactions_json(idCstr, kindCstr)
            }
        }
        guard let ptr else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return [] }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return (try? decoder.decode([QuickActionDescriptor].self, from: data)) ?? []
    }

    /// The entity from a definitional query ("what is vim" -> "vim"), or nil.
    /// Network-free heuristic in the Rust core.
    nonisolated func definitionalEntity(query: String) -> String? {
        let ptr = query.withCString { look_definitional_entity_json($0) }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        let raw = String(cString: ptr)
        guard raw != "null", let data = raw.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(String.self, from: data)
    }

    /// Decodes a `look_answers::Answer` JSON C string (or `null`) into a
    /// `WebAnswer`, freeing the pointer. Shared by the instant/DDG/Wikipedia
    /// paths since they all return the same shape.
    nonisolated private func decodeWebAnswer(_ ptr: UnsafeMutablePointer<CChar>?) -> WebAnswer? {
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }

        let raw = String(cString: ptr)
        guard raw != "null", let data = raw.data(using: .utf8) else { return nil }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        guard let dto = try? decoder.decode(AnswerDTO.self, from: data) else { return nil }
        return WebAnswer(
            text: dto.text,
            source: dto.source,
            url: dto.url.flatMap(URL.init(string:)),
            imageURL: dto.imageUrl.flatMap(URL.init(string:))
        )
    }

    nonisolated private func fallbackResults() -> [LauncherResult] {
        []
    }
}

/// Wire shape of a `look_answers::Answer` JSON object (snake_case `image_url`
/// decoded via `.convertFromSnakeCase`).
private nonisolated struct AnswerDTO: Decodable {
    let text: String
    let source: String
    let url: String?
    let imageUrl: String?
}

nonisolated struct TranslationResult: Decodable {
    let original: String
    let translated: String
    let error: BridgeError?
}

/// Wire shape of `look_answers::UrlMatch`: the resolved openable URL and how
/// certain the classification is. `tier` decodes the lowercased Rust enum
/// (`structural` / `barehost`).
nonisolated struct URLMatch: Decodable {
    enum Tier: String, Decodable {
        case structural
        case bareHost = "barehost"
    }

    let url: String
    let tier: Tier
}

/// Wire shape of a `url_history` row (see url-history spec), decoded with
/// `.convertFromSnakeCase`. `title` is reserved and nil today.
nonisolated struct URLHistoryEntry: Decodable {
    let url: String
    let title: String?
    let hitCount: Int
    let lastUsedAtUnixS: Int
    /// Frecency rank from the Rust core (same `rank_score` as apps/files), used
    /// to place recent URLs among local results rather than a fixed threshold.
    let score: Int
}

private nonisolated struct SearchPayload: Decodable {
    let query: String
    let count: Int
    let results: [SearchItem]
    let error: BridgeError?
}

private nonisolated struct CompactSearchPayload: Decodable {
    let count: Int
    let results: [SearchItem]
    let error: BridgeError?
}

private nonisolated struct UsageRecordPayload: Decodable {
    let ok: Bool
    let error: BridgeError?
}

nonisolated struct BridgeError: Decodable {
    let code: String
    let message: String

    var userFacingMessage: String {
        BridgeErrorMapping.userFacingMessage(code: code, fallback: message)
    }
}

private nonisolated struct SearchItem: Decodable {
    let id: String
    let kind: String
    let title: String
    let subtitle: String?
    let path: String
    let score: Int
}
