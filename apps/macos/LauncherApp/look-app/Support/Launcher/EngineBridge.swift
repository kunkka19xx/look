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

@_silgen_name("look_fuzzy_score")
nonisolated
private func look_fuzzy_score(_ query: UnsafePointer<CChar>?, _ title: UnsafePointer<CChar>?) -> Int64

@_silgen_name("look_instant_answer_json")
nonisolated
private func look_instant_answer_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_is_referent")
nonisolated
private func look_ai_is_referent(_ phrase: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_ai_query_window")
nonisolated
private func look_ai_query_window(_ query: UnsafePointer<CChar>?, _ nowEpoch: Int64) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_plan")
nonisolated
private func look_ai_plan(_ host: UnsafePointer<CChar>?, _ model: UnsafePointer<CChar>?, _ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_warm_planner")
nonisolated
private func look_ai_warm_planner(_ host: UnsafePointer<CChar>?, _ model: UnsafePointer<CChar>?)

@_silgen_name("look_ai_conversations_json")
nonisolated
private func look_ai_conversations_json(_ path: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_conversation_upsert")
nonisolated
private func look_ai_conversation_upsert(_ path: UnsafePointer<CChar>?, _ json: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_ai_resolve")
nonisolated
private func look_ai_resolve(_ requestJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_parse_explicit")
nonisolated
private func look_ai_parse_explicit(_ input: UnsafePointer<CChar>?, _ modelAvailable: Bool) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_chat_start")
nonisolated
private func look_ai_chat_start(_ host: UnsafePointer<CChar>?, _ model: UnsafePointer<CChar>?, _ messagesJSON: UnsafePointer<CChar>?) -> UInt64

@_silgen_name("look_ai_chat_poll")
nonisolated
private func look_ai_chat_poll(_ id: UInt64) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_chat_cancel")
nonisolated
private func look_ai_chat_cancel(_ id: UInt64)

@_silgen_name("look_ai_markdown_segments_json")
nonisolated
private func look_ai_markdown_segments_json(_ text: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_instant_has_match")
nonisolated
private func look_instant_has_match(_ query: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_calc_eval_json")
nonisolated
private func look_calc_eval_json(_ expr: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_calc_inline_json")
nonisolated
private func look_calc_inline_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

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

@_silgen_name("look_quick_actions_launchpad_json")
nonisolated
private func look_quick_actions_launchpad_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_definitional_entity_json")
nonisolated
private func look_definitional_entity_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_todo_list_json")
nonisolated
private func look_todo_list_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_todo_save_json")
nonisolated
private func look_todo_save_json(_ json: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_lunar_date_json")
nonisolated
private func look_lunar_date_json(_ year: Int64, _ month: Int64, _ day: Int64, _ tz: Double) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_netspeed_run_json")
nonisolated
private func look_netspeed_run_json() -> UnsafeMutablePointer<CChar>?

/// One measurement from the shared `core/netspeed` crate. The display strings
/// are formatted in core so every shell prints the same text.
nonisolated struct SpeedReading: Codable, Sendable, Equatable {
    let downloadBitsPerSecond: Double
    let uploadBitsPerSecond: Double
    let latencyMs: Double?
    let downloadDisplay: String
    let uploadDisplay: String
    let latencyDisplay: String
    let downloadVerdict: String
    let latencyVerdict: String
    let latencyLevel: String?
    let downloadSource: String?
    var publicIp: String?
    let provider: String?
    let location: String?
    let measuredAtUnix: Int

    var measuredAt: Date {
        Date(timeIntervalSince1970: TimeInterval(measuredAtUnix))
    }
}

/// Stands in when the bridge cannot make sense of the reply at all; every other
/// message the panel shows comes from core's `SpeedError`.
private nonisolated let speedTestUnknownFailure = "Speed test failed"

nonisolated struct SpeedTestEnvelope: Decodable {
    let ok: Bool
    let reading: SpeedReading?
    let error: String?
}

nonisolated enum SpeedTestOutcome: Sendable {
    case reading(SpeedReading)
    case failure(String)
}

/// A resolved lunar date from the shared `core/lunar` crate (East Asian
/// lunisolar calendar). `leap` marks the intercalary month of a 13-month year.
nonisolated struct LunarDate: Decodable {
    let day: Int
    let month: Int
    let year: Int
    let leap: Bool
}

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

    /// Lunar date from the shared core. `tzHours` is the viewer's UTC offset,
    /// which selects the calendar variant (7 = Vietnamese, 8 = Chinese).
    nonisolated func lunarDate(year: Int, month: Int, day: Int, tzHours: Double) -> LunarDate? {
        guard let ptr = look_lunar_date_json(Int64(year), Int64(month), Int64(day), tzHours) else {
            return nil
        }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(LunarDate.self, from: data)
    }

    /// Runs the shared core speed test. Blocks for 15 seconds and up, so call it
    /// off the main thread. There is no cancel: core bounds each phase with its
    /// own timeout.
    nonisolated func speedTest() -> SpeedTestOutcome {
        guard let ptr = look_netspeed_run_json() else {
            return .failure(speedTestUnknownFailure)
        }
        defer { look_free_cstring(ptr) }

        guard let data = String(cString: ptr).data(using: .utf8) else {
            return .failure(speedTestUnknownFailure)
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        guard let envelope = try? decoder.decode(SpeedTestEnvelope.self, from: data) else {
            return .failure(speedTestUnknownFailure)
        }
        guard envelope.ok, let reading = envelope.reading else {
            return .failure(envelope.error ?? speedTestUnknownFailure)
        }
        return .reading(reading)
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

    /// Shared `core/matching` fuzzy score for `query` vs `title` (identical
    /// ranking to linows), or nil on no match. Both sides must be pre-lowercased
    /// by the caller (the scorer is case-sensitive). Cheap - safe while typing.
    /// One chat-answer chunk from the Rust-core markdown segmentation (core/ai).
    struct AIMarkdownSegment: Decodable, Equatable {
        let kind: String  // "text" | "code"
        let text: String
        let language: String?
    }

    /// Whether a mutate `match` phrase refers to conversation context ("it",
    /// "this event") rather than naming an item. Rust core (core/ai).
    nonisolated func aiIsReferent(_ phrase: String) -> Bool {
        phrase.withCString { look_ai_is_referent($0) }
    }

    /// Timeframe a schedule question names ("next week", "in august"), from the
    /// Rust-core window grammar (core/ai). ISO Monday weeks, local midnights.
    nonisolated func aiQueryWindow(_ query: String) -> (start: Date, end: Date, label: String)? {
        struct RawWindow: Decodable {
            let start: Int64
            let end: Int64
            let label: String
        }
        let now = Int64(Date().timeIntervalSince1970)
        guard let ptr = query.withCString({ look_ai_query_window($0, now) }) else { return nil }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let window = try? JSONDecoder().decode(RawWindow.self, from: data)
        else { return nil }
        return (
            Date(timeIntervalSince1970: TimeInterval(window.start)),
            Date(timeIntervalSince1970: TimeInterval(window.end)),
            window.label)
    }

    /// One planning call to the local model via the Rust-core planner
    /// (core/ai): the prompt, aliases, and mapping live there. BLOCKING
    /// network; callers dispatch off the main thread.
    nonisolated func aiPlan(host: String, model: String, query: String) -> (toolID: String, params: [String: String])? {
        struct RawCall: Decodable {
            let tool: String
            let params: [String: String]
        }
        let ptr = host.withCString { hostC in
            model.withCString { modelC in
                query.withCString { queryC in
                    look_ai_plan(hostC, modelC, queryC)
                }
            }
        }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let call = try? JSONDecoder().decode(RawCall.self, from: data)
        else { return nil }
        return (call.tool, call.params)
    }

    /// Primes the model + prompt cache (BLOCKING network; call off-thread).
    nonisolated func aiWarmPlanner(host: String, model: String) {
        host.withCString { hostC in
            model.withCString { modelC in
                look_ai_warm_planner(hostC, modelC)
            }
        }
    }

    /// Stored AI conversations (newest first) from the Rust-core store.
    nonisolated func aiConversationsJSON(path: String) -> Data? {
        guard let ptr = path.withCString({ look_ai_conversations_json($0) }) else { return nil }
        defer { look_free_cstring(ptr) }
        return String(cString: ptr).data(using: .utf8)
    }

    /// Insert-or-replace one conversation in the Rust-core store.
    nonisolated func aiConversationUpsert(path: String, json: String) -> Bool {
        path.withCString { pathC in
            json.withCString { jsonC in
                look_ai_conversation_upsert(pathC, jsonC)
            }
        }
    }

    /// Tool resolution via the Rust core (core/ai): candidates + params in,
    /// a data-only planned/choice/invalid outcome out. Pure CPU.
    nonisolated func aiResolve(requestJSON: String) -> Data? {
        guard let ptr = requestJSON.withCString({ look_ai_resolve($0) }) else { return nil }
        defer { look_free_cstring(ptr) }
        return String(cString: ptr).data(using: .utf8)
    }

    /// The explicit `>verb title @ when` parser (Rust core). Nil = natural
    /// language, deferred to the model.
    nonisolated func aiParseExplicit(_ input: String, modelAvailable: Bool) -> (toolID: String, params: [String: String])? {
        struct RawCall: Decodable {
            let tool: String
            let params: [String: String]
        }
        let ptr = input.withCString { look_ai_parse_explicit($0, modelAvailable) }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let call = try? JSONDecoder().decode(RawCall.self, from: data)
        else { return nil }
        return (call.tool, call.params)
    }

    nonisolated struct AIChatSnapshot: Decodable {
        let text: String
        let done: Bool
        let error: String?
    }

    /// Start a streamed chat session in the Rust core (curl child). 0 = failed.
    nonisolated func aiChatStart(host: String, model: String, messagesJSON: String) -> UInt64 {
        host.withCString { hostC in
            model.withCString { modelC in
                messagesJSON.withCString { messagesC in
                    look_ai_chat_start(hostC, modelC, messagesC)
                }
            }
        }
    }

    /// Snapshot of a chat session's cumulative text; nil for unknown ids.
    nonisolated func aiChatPoll(_ id: UInt64) -> AIChatSnapshot? {
        guard let ptr = look_ai_chat_poll(id) else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(AIChatSnapshot.self, from: data)
    }

    /// Abort a chat session (Ollama stops generating).
    nonisolated func aiChatCancel(_ id: UInt64) {
        look_ai_chat_cancel(id)
    }

    /// Markdown segmentation for AI chat answers. Rust core (core/ai).
    nonisolated func aiMarkdownSegments(_ text: String) -> [AIMarkdownSegment] {
        guard let ptr = text.withCString({ look_ai_markdown_segments_json($0) }) else { return [] }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let segments = try? JSONDecoder().decode([AIMarkdownSegment].self, from: data)
        else { return [] }
        return segments
    }

    nonisolated func fuzzyScore(query: String, title: String) -> Int? {
        let score = query.withCString { queryCstr in
            title.withCString { titleCstr in
                look_fuzzy_score(queryCstr, titleCstr)
            }
        }
        return score == Int64.min ? nil : Int(score) // Int64.min = NO_MATCH sentinel
    }

    /// Network-free gate: whether `query` matches a shared instant-answer
    /// provider (currency/weather/crypto). Cheap - safe to call while typing.
    nonisolated func instantAnswerMatches(_ query: String) -> Bool {
        query.withCString { look_instant_has_match($0) }
    }

    /// Evaluates `expr` as arithmetic via the shared `core/calc` engine - the
    /// dedicated `/calc` panel, where the user already declared this is a
    /// calculation and a specific error is worth showing. Network-free; safe
    /// to call while typing.
    nonisolated func calcEval(expr: String) -> CalcEvalResult {
        let fallback = CalcEvalResult(calculation: nil, error: "Invalid expression")
        guard let ptr = expr.withCString({ look_calc_eval_json($0) }) else { return fallback }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8),
            let result = try? JSONDecoder().decode(CalcEvalResult.self, from: data)
        else { return fallback }
        return result
    }

    /// The main search field: resolves `query` only when it was clearly meant
    /// as arithmetic (dates/resolutions/ratios stay untouched). Network-free
    /// and cheap enough to call on every keystroke.
    nonisolated func calcInline(query: String) -> CalculationDTO? {
        decodeCalculation(query.withCString { look_calc_inline_json($0) })
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

    /// The empty-state launchpad layout from the shared `look_qactions` catalog:
    /// fixed tile order, sizes, and mnemonics. Pure catalog lookup, cheap. Empty
    /// only on an unexpected decode failure.
    nonisolated func launchpadLayout() -> [LaunchpadTileModel] {
        guard let ptr = look_quick_actions_launchpad_json() else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return [] }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return (try? decoder.decode([LaunchpadTileModel].self, from: data)) ?? []
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

    /// Decodes a `look_calc::Calculation` JSON C string (or `null`), freeing
    /// the pointer.
    nonisolated private func decodeCalculation(_ ptr: UnsafeMutablePointer<CChar>?) -> CalculationDTO? {
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }

        let raw = String(cString: ptr)
        guard raw != "null", let data = raw.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(CalculationDTO.self, from: data)
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

/// Wire shape of a `look_calc::Calculation` JSON object: `display` is grouped
/// for showing (`1,000,000`), `raw` is bare and re-parseable, for the clipboard
/// (`1000000`).
nonisolated struct CalculationDTO: Decodable {
    let display: String
    let raw: String
    let value: Double
}

/// Wire shape of `look_calc_eval_json`: exactly one of the two is non-nil.
nonisolated struct CalcEvalResult: Decodable {
    let calculation: CalculationDTO?
    let error: String?
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
