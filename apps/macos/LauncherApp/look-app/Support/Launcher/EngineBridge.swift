import Foundation

@_silgen_name("look_search_json")
nonisolated
private func look_search_json(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_search_json_compact")
nonisolated
private func look_search_json_compact(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_search_files_json")
nonisolated
private func look_search_files_json(_ query: UnsafePointer<CChar>?, _ now: Int64, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

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

@_silgen_name("look_search_files_params_json")
nonisolated
private func look_search_files_params_json(_ paramsJSON: UnsafePointer<CChar>?, _ now: Int64, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_query_window")
nonisolated
private func look_ai_query_window(_ query: UnsafePointer<CChar>?, _ nowEpoch: Int64) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_day_phrase")
nonisolated
private func look_ai_day_phrase(_ phrase: UnsafePointer<CChar>?, _ nowEpoch: Int64) -> Int64

@_silgen_name("look_ai_plan_start")
nonisolated
private func look_ai_plan_start(_ host: UnsafePointer<CChar>?, _ model: UnsafePointer<CChar>?, _ query: UnsafePointer<CChar>?) -> UInt64

@_silgen_name("look_ai_plan_poll")
nonisolated
private func look_ai_plan_poll(_ id: UInt64) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_plan_cancel")
nonisolated
private func look_ai_plan_cancel(_ id: UInt64)

@_silgen_name("look_ai_warm_planner")
nonisolated
private func look_ai_warm_planner(_ host: UnsafePointer<CChar>?, _ model: UnsafePointer<CChar>?)

@_silgen_name("look_ai_conversations_json")
nonisolated
private func look_ai_conversations_json(_ path: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_conversation_upsert")
nonisolated
private func look_ai_conversation_upsert(_ path: UnsafePointer<CChar>?, _ json: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_ai_conversation_delete")
nonisolated
private func look_ai_conversation_delete(_ path: UnsafePointer<CChar>?, _ id: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_ai_resolve")
nonisolated
private func look_ai_resolve(_ requestJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_load_targets")
nonisolated
private func look_ai_load_targets(_ eventsJSON: UnsafePointer<CChar>?, _ remindersJSON: UnsafePointer<CChar>?)

@_silgen_name("look_ai_memory_context")
nonisolated
private func look_ai_memory_context(_ path: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_context_window")
nonisolated
private func look_ai_context_window(_ textsJSON: UnsafePointer<CChar>?, _ budget: UInt32) -> UInt32

@_silgen_name("look_ai_parse_explicit")
nonisolated
private func look_ai_parse_explicit(_ input: UnsafePointer<CChar>?, _ modelAvailable: Bool) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_route")
nonisolated
private func look_ai_route(_ memoryPath: UnsafePointer<CChar>?, _ input: UnsafePointer<CChar>?, _ modelAvailable: Bool, _ now: Int64) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_meeting_join_query_json")
nonisolated
private func look_meeting_join_query_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_call_query_json")
nonisolated
private func look_call_query_json(_ query: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_call_url")
nonisolated
private func look_call_url(_ modality: UnsafePointer<CChar>?, _ handle: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_call_default_modality")
nonisolated
private func look_call_default_modality() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_meeting_outcome_json")
nonisolated
private func look_meeting_outcome_json(_ eventsJSON: UnsafePointer<CChar>?, _ now: Int64, _ name: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_chat_start")
nonisolated
private func look_ai_chat_start(_ host: UnsafePointer<CChar>?, _ model: UnsafePointer<CChar>?, _ messagesJSON: UnsafePointer<CChar>?, _ optionsJSON: UnsafePointer<CChar>?) -> UInt64

@_silgen_name("look_ai_chat_poll")
nonisolated
private func look_ai_chat_poll(_ id: UInt64) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_ai_chat_cancel")
nonisolated
private func look_ai_chat_cancel(_ id: UInt64)

@_silgen_name("look_ai_future_leaning")
nonisolated
private func look_ai_future_leaning(_ phrase: UnsafePointer<CChar>?, _ resolvedEpoch: Int64, _ nowEpoch: Int64) -> Int64

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

@_silgen_name("look_clipboard_record")
nonisolated
private func look_clipboard_record(_ content: UnsafePointer<CChar>?, _ kind: UnsafePointer<CChar>?, _ appBundleID: UnsafePointer<CChar>?) -> Int64

@_silgen_name("look_clipboard_list_json")
nonisolated
private func look_clipboard_list_json(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_clipboard_delete")
nonisolated
private func look_clipboard_delete(_ id: Int64) -> Bool

@_silgen_name("look_clipboard_clear")
nonisolated
private func look_clipboard_clear() -> UInt32

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

@_silgen_name("look_refresh_run_blocks_json")
nonisolated
private func look_refresh_run_blocks_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_source_preview_json")
nonisolated
private func look_source_preview_json(_ candidateID: UnsafePointer<CChar>?, _ rowID: UnsafePointer<CChar>?, _ rowTitle: UnsafePointer<CChar>?, _ rowPath: UnsafePointer<CChar>?, _ ancestorsJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_source_blocks_json")
nonisolated
private func look_source_blocks_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_source_rows_json")
nonisolated
private func look_source_rows_json(_ blockID: UnsafePointer<CChar>?, _ parentCandidateID: UnsafePointer<CChar>?, _ parentTitle: UnsafePointer<CChar>?, _ parentPath: UnsafePointer<CChar>?, _ query: UnsafePointer<CChar>?, _ ancestorsJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_source_block_json")
nonisolated
private func look_source_block_json(_ candidateID: UnsafePointer<CChar>?, _ rowID: UnsafePointer<CChar>?, _ rowTitle: UnsafePointer<CChar>?, _ rowPath: UnsafePointer<CChar>?, _ ancestorsJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_perform_block_json")
nonisolated
private func look_perform_block_json(_ blockID: UnsafePointer<CChar>?, _ rowID: UnsafePointer<CChar>?, _ rowTitle: UnsafePointer<CChar>?, _ rowPath: UnsafePointer<CChar>?, _ query: UnsafePointer<CChar>?, _ ancestorsJSON: UnsafePointer<CChar>?, _ asTarget: Bool) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_tool_action_json")
nonisolated
private func look_tool_action_json(_ action: UnsafePointer<CChar>?, _ candidateID: UnsafePointer<CChar>?, _ rowTitle: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?, _ isDir: Bool, _ ancestorsJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_perform_tool_action_json")
nonisolated
private func look_perform_tool_action_json(_ action: UnsafePointer<CChar>?, _ candidateID: UnsafePointer<CChar>?, _ rowTitle: UnsafePointer<CChar>?, _ path: UnsafePointer<CChar>?, _ isDir: Bool, _ ancestorsJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

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
    nonisolated static let shared = EngineBridge()

    nonisolated private init() {}

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
            return compactPayload.results.map { LauncherResult($0, defaultKind: .app) }
        }

        // Compatibility fallback for older JSON payload shape.
        guard let fullPayload = try? JSONDecoder().decode(SearchPayload.self, from: data),
            fullPayload.error == nil
        else {
            return fallbackResults()
        }

        return fullPayload.results.map { LauncherResult($0, defaultKind: .app) }
    }

    /// The Rust-core routing decision for submitted AI-mode input (see
    /// core/ai/src/route.rs: memory -> join -> textop -> files -> explicit ->
    /// plan -> chat). The memory tier has already executed by the time this
    /// returns.
    enum AIRoute {
        case memory(feedback: String)
        /// "join", "join my next meeting", "join <name>". The shell resolves
        /// the name against the calendar.
        case join(name: String?)
        /// "call mom", "facetime sarah". `modality` is a `Modality` id, or nil
        /// when the words did not say and the default applies.
        case call(name: String, modality: String?)
        case textOp(label: String, instruction: String)
        case files
        case explicit(toolID: String, params: [String: String])
        case plan
        case chat
    }

    nonisolated func aiRoute(input: String, memoryPath: String, modelAvailable: Bool) -> AIRoute {
        struct Payload: Decodable {
            struct Call: Decodable {
                let tool: String
                let params: [String: String]
            }
            let route: String
            let feedback: String?
            let label: String?
            let instruction: String?
            let call: Call?
            /// The join tier's meeting name (absent for a bare "join"), or the
            /// call tier's person.
            let name: String?
            /// The call tier's modality id, absent when the words did not say.
            let modality: String?
        }
        let now = Int64(Date().timeIntervalSince1970)
        let ptr = memoryPath.withCString { pathC in
            input.withCString { inputC in
                look_ai_route(pathC, inputC, modelAvailable, now)
            }
        }
        guard let ptr else { return .chat }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let payload = try? JSONDecoder().decode(Payload.self, from: data)
        else { return .chat }
        switch payload.route {
        case "memory":
            return .memory(feedback: payload.feedback ?? "")
        case "join":
            return .join(name: payload.name)
        case "call":
            guard let name = payload.name, !name.isEmpty else { return .chat }
            return .call(name: name, modality: payload.modality)
        case "textop":
            guard let instruction = payload.instruction, !instruction.isEmpty else { return .chat }
            return .textOp(label: payload.label ?? instruction, instruction: instruction)
        case "files":
            return .files
        case "explicit":
            guard let call = payload.call else { return .chat }
            return .explicit(toolID: call.tool, params: call.params)
        case "plan":
            return .plan
        default:
            return .chat
        }
    }

    /// File-recall results plus which fallback produced them, when the strict
    /// query matched nothing ("window" | "terms" | "window_terms", nil = exact).
    nonisolated struct FileRecallOutcome {
        let results: [LauncherResult]
        let relaxed: String?
    }

    /// The join request in the typed text, or nil when it is an ordinary
    /// search. Pure string work in core, so it is safe per keystroke.
    nonisolated func joinQuery(_ query: String) -> JoinRequest? {
        guard let ptr = query.withCString({ look_meeting_join_query_json($0) }) else {
            return nil
        }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(JoinRequest.self, from: data)
    }

    /// The call request in the typed text, or nil when it is an ordinary
    /// search. Pure string work in core, so it is safe per keystroke.
    nonisolated func callQuery(_ query: String) -> CallRequest? {
        guard let ptr = query.withCString({ look_call_query_json($0) }) else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        // Core answers the literal `null` for a non-call, which fails to decode
        // into a non-optional and so becomes the nil this returns anyway.
        return try? JSONDecoder().decode(CallRequest.self, from: data)
    }

    /// The URL that dials `handle` with `modality`, or nil when the modality
    /// is unknown to core. Building it there keeps the schemes in one place.
    nonisolated func callURL(modality: String, handle: String) -> String? {
        guard
            let ptr = modality.withCString({ modalityC in
                handle.withCString { look_call_url(modalityC, $0) }
            })
        else { return nil }
        defer { look_free_cstring(ptr) }
        let url = String(cString: ptr)
        return url.isEmpty ? nil : url
    }

    /// The modality a bare "call" means, straight from core.
    nonisolated var defaultCallModality: String {
        guard let ptr = look_call_default_modality() else { return "" }
        defer { look_free_cstring(ptr) }
        return String(cString: ptr)
    }

    /// What a `join` finds in `eventsJSON`: the meetings it can open, best
    /// first, plus the titles that matched the name but carry no link. `name`
    /// narrows to meetings whose title holds those words. The ordering, and
    /// where a join link hides inside an event, are decided in core
    /// (`look_ai::meeting`) so every shell agrees.
    nonisolated func joinOutcome(
        eventsJSON: String, now: Int64, name: String = ""
    ) -> JoinOutcome {
        guard
            let ptr = eventsJSON.withCString({ events in
                name.withCString { look_meeting_outcome_json(events, now, $0) }
            })
        else {
            return JoinOutcome()
        }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return JoinOutcome() }
        return (try? JSONDecoder().decode(JoinOutcome.self, from: data)) ?? JoinOutcome()
    }

    /// Natural-language file recall over Look's own index. Returns nil when the
    /// query is not a file-recall query (so the caller does normal search).
    nonisolated func searchFiles(query: String, limit: Int = 40) -> FileRecallOutcome? {
        let now = Int64(Date().timeIntervalSince1970)
        guard let ptr = query.withCString({ look_search_files_json($0, now, UInt32(limit)) }) else {
            return nil
        }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let payload = try? JSONDecoder().decode(SearchPayload.self, from: data),
            payload.error == nil
        else { return nil }
        return Self.fileRecallOutcome(from: payload)
    }

    /// File recall from the model's structured `recall` params (terms, types,
    /// when, location). Nil when the params are unusable.
    nonisolated func searchFiles(params: [String: String], limit: Int = 40) -> FileRecallOutcome? {
        guard
            let paramsData = try? JSONSerialization.data(withJSONObject: params),
            let paramsJSON = String(data: paramsData, encoding: .utf8)
        else { return nil }
        let now = Int64(Date().timeIntervalSince1970)
        guard let ptr = paramsJSON.withCString({ look_search_files_params_json($0, now, UInt32(limit)) }) else {
            return nil
        }
        defer { look_free_cstring(ptr) }
        guard
            let data = String(cString: ptr).data(using: .utf8),
            let payload = try? JSONDecoder().decode(SearchPayload.self, from: data),
            payload.error == nil
        else { return nil }
        return Self.fileRecallOutcome(from: payload)
    }

    private nonisolated static func fileRecallOutcome(from payload: SearchPayload) -> FileRecallOutcome {
        let results = payload.results.map { LauncherResult($0, defaultKind: .file) }
        return FileRecallOutcome(results: results, relaxed: payload.relaxed)
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

    /// The specific day a phrase names ("wed", "tmr", "last fri"), from the
    /// shared lexicon - the fallback behind NSDataDetector so abbreviations
    /// resolve the same on every shell. Local midnight; nil when none named.
    nonisolated func aiDayPhrase(_ phrase: String) -> Date? {
        let now = Int64(Date().timeIntervalSince1970)
        let epoch = phrase.withCString { look_ai_day_phrase($0, now) }
        guard epoch > 0 else { return nil }
        return Date(timeIntervalSince1970: TimeInterval(epoch))
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

    /// One line of a planning session poll: pending until `done`, then the
    /// resolved call (nil call = not an action / failure).
    nonisolated struct AIPlanSnapshot: Decodable {
        struct RawCall: Decodable {
            let tool: String
            let params: [String: String]
        }
        let done: Bool
        /// Every step of the plan, in order. Empty when the request was not an
        /// action; several for a compound one.
        let calls: [RawCall]?

        var steps: [RawCall] { calls ?? [] }
    }

    /// Starts a cancellable planning call via the Rust-core planner (core/ai):
    /// the prompt, aliases, and mapping live there. Returns 0 on failure.
    nonisolated func aiPlanStart(host: String, model: String, query: String) -> UInt64 {
        host.withCString { hostC in
            model.withCString { modelC in
                query.withCString { queryC in
                    look_ai_plan_start(hostC, modelC, queryC)
                }
            }
        }
    }

    /// Snapshot of a planning session; nil for unknown ids. The poll that
    /// observes `done` removes the session.
    nonisolated func aiPlanPoll(_ id: UInt64) -> AIPlanSnapshot? {
        guard let ptr = look_ai_plan_poll(id) else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(AIPlanSnapshot.self, from: data)
    }

    /// Kills the planning request (Ollama aborts generation on disconnect).
    nonisolated func aiPlanCancel(_ id: UInt64) {
        look_ai_plan_cancel(id)
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

    nonisolated func aiConversationDelete(path: String, id: String) -> Bool {
        path.withCString { pathC in
            id.withCString { idC in
                look_ai_conversation_delete(pathC, idC)
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

    /// Load the AI mutate targets once (events + reminders JSON arrays) so
    /// per-keystroke `aiResolve` calls can omit the lists. Rust core.
    nonisolated func aiLoadTargets(eventsJSON: String, remindersJSON: String) {
        eventsJSON.withCString { ev in
            remindersJSON.withCString { rem in
                look_ai_load_targets(ev, rem)
            }
        }
    }

    /// Stored facts as a model context block (empty when none). Rust core.
    nonisolated func aiMemoryContext(path: String) -> String {
        guard let ptr = path.withCString({ look_ai_memory_context($0) }) else { return "" }
        defer { look_free_cstring(ptr) }
        return String(cString: ptr)
    }

    /// How many recent item texts fit the token budget (0 = default). The shell
    /// keeps that many tail turns as chat history. Rust core (core/ai/context).
    nonisolated func aiContextWindow(texts: [String], budget: Int = 0) -> Int {
        guard
            let data = try? JSONSerialization.data(withJSONObject: texts),
            let json = String(data: data, encoding: .utf8)
        else { return texts.count }
        let count = json.withCString { look_ai_context_window($0, UInt32(max(0, budget))) }
        return Int(count)
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
        /// The answer hit the generation length cap and was cut off.
        let truncated: Bool?
    }

    /// Start a streamed chat session in the Rust core (curl child). 0 = failed.
    /// `optionsJSON` tunes the generation for this surface
    /// (`{num_predict, temperature, timeout_secs}`); empty for core defaults.
    nonisolated func aiChatStart(
        host: String, model: String, messagesJSON: String, optionsJSON: String = ""
    ) -> UInt64 {
        host.withCString { hostC in
            model.withCString { modelC in
                messagesJSON.withCString { messagesC in
                    optionsJSON.withCString { optionsC in
                        look_ai_chat_start(hostC, modelC, messagesC, optionsC)
                    }
                }
            }
        }
    }

    /// Streams an Ollama answer over the shared Rust chat transport, yielding
    /// cumulative text (the `AIQueryProvider.answer` contract). One transport
    /// for every Ollama caller: same cancellation, timeout, and error handling
    /// as session chat. Cancelling the consuming task kills the request.
    nonisolated func aiChatStream(
        host: String, model: String, messagesJSON: String, optionsJSON: String
    ) -> AsyncThrowingStream<String, Error>? {
        let session = aiChatStart(
            host: host, model: model, messagesJSON: messagesJSON, optionsJSON: optionsJSON)
        guard session != 0 else { return nil }
        return AsyncThrowingStream { continuation in
            let task = Task.detached(priority: .userInitiated) {
                defer { if Task.isCancelled { EngineBridge.shared.aiChatCancel(session) } }
                while !Task.isCancelled {
                    try? await Task.sleep(nanoseconds: 85_000_000)
                    if Task.isCancelled { return }
                    guard let snapshot = EngineBridge.shared.aiChatPoll(session) else {
                        continuation.finish()
                        return
                    }
                    if !snapshot.text.isEmpty {
                        continuation.yield(snapshot.text)
                    }
                    if let error = snapshot.error {
                        continuation.finish(throwing: OllamaError.server(error))
                        return
                    }
                    if snapshot.done {
                        continuation.finish()
                        return
                    }
                }
            }
            continuation.onTermination = { _ in task.cancel() }
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

    /// Rolls a time-only phrase forward when it already passed today (Rust core).
    nonisolated func aiFutureLeaning(phrase: String, resolved: Date, now: Date) -> Date {
        let adjusted = phrase.withCString {
            look_ai_future_leaning($0, Int64(resolved.timeIntervalSince1970), Int64(now.timeIntervalSince1970))
        }
        return Date(timeIntervalSince1970: TimeInterval(adjusted))
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

    /// The user-declared block a row belongs to, with the exact steps Enter will
    /// perform. Reads the sources directory, so call it off the main thread.
    nonisolated func sourceBlock(
        candidateID: String, rowID: String = "", rowTitle: String = "", rowPath: String = "",
        ancestorsJSON: String = "[]"
    ) -> SourceBlock? {
        let ptr = candidateID.withCString { candidate in
            rowID.withCString { id in
                rowTitle.withCString { title in
                    rowPath.withCString { path in
                        ancestorsJSON.withCString { ancestors in
                            look_source_block_json(candidate, id, title, path, ancestors)
                        }
                    }
                }
            }
        }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SourceBlock.self, from: data)
    }

    /// The rows of `blockID`, produced against the row a level is opening from.
    /// Runs the block's command, so call it off the main thread. `error`
    /// non-nil means do not descend.
    nonisolated func sourceRows(
        blockID: String, parentCandidateID: String, parentTitle: String, parentPath: String,
        query: String, ancestorsJSON: String = "[]"
    ) -> SourceLevel? {
        let ptr = blockID.withCString { block in
            parentCandidateID.withCString { parent in
                parentTitle.withCString { title in
                    parentPath.withCString { path in
                        query.withCString { query in
                            ancestorsJSON.withCString { ancestors in
                                look_source_rows_json(
                                    block, parent, title, path, query, ancestors)
                            }
                        }
                    }
                }
            }
        }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SourceLevel.self, from: data)
    }

    /// What `action` would do to a row, without doing it: the tool it would
    /// start, or why it cannot. For labels and availability.
    nonisolated func toolAction(
        _ action: String, row: ToolActionRow, isDirectory: Bool
    ) -> ToolAction? {
        let ptr = row.withCStrings { candidate, title, path, ancestors in
            action.withCString { action in
                look_tool_action_json(action, candidate, title, path, isDirectory, ancestors)
            }
        }
        return Self.decodeToolAction(ptr)
    }

    /// Runs `action` on a row. Shell actions are spawned detached inside core;
    /// an `application` result is handed back for `NSWorkspace` to launch.
    /// Spawns a process, so call it off the main thread.
    nonisolated func performToolAction(
        _ action: String, row: ToolActionRow, isDirectory: Bool
    ) -> ToolAction? {
        let ptr = row.withCStrings { candidate, title, path, ancestors in
            action.withCString { action in
                look_perform_tool_action_json(action, candidate, title, path, isDirectory, ancestors)
            }
        }
        return Self.decodeToolAction(ptr)
    }

    private nonisolated static func decodeToolAction(_ ptr: UnsafeMutablePointer<CChar>?) -> ToolAction? {
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(ToolAction.self, from: data)
    }

    /// Re-runs every `run` block and stores its rows for the next index pass.
    /// Spawns the user's commands and waits for them - call off the main thread.
    nonisolated func refreshRunBlocks() -> RunBlockRefreshOutcome {
        guard let ptr = look_refresh_run_blocks_json() else {
            return RunBlockRefreshOutcome(refreshed: 0, errors: [])
        }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8),
              let outcome = try? JSONDecoder().decode(RunBlockRefreshOutcome.self, from: data)
        else {
            return RunBlockRefreshOutcome(refreshed: 0, errors: [])
        }
        return outcome
    }

    /// A block's declared `preview`, run against the selected row. Nil when the
    /// block declares none. Runs a command - call off the main thread.
    nonisolated func sourcePreview(
        candidateID: String, rowID: String, rowTitle: String, rowPath: String,
        ancestorsJSON: String = "[]"
    ) -> SourcePreview? {
        let ptr = candidateID.withCString { candidate in
            rowID.withCString { id in
                rowTitle.withCString { title in
                    rowPath.withCString { path in
                        ancestorsJSON.withCString { ancestors in
                            look_source_preview_json(candidate, id, title, path, ancestors)
                        }
                    }
                }
            }
        }
        guard let ptr else { return nil }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SourcePreview.self, from: data)
    }

    /// Every declared block, for the row-icon cache. Reads the sources
    /// directory, so call it off the main thread.
    nonisolated func sourceBlocks() -> [SourceBlockSummary] {
        guard let ptr = look_source_blocks_json() else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([SourceBlockSummary].self, from: data)) ?? []
    }

    /// Performs every step of that block, detached, through the user's login
    /// shell. Spawns processes - call off the main thread.
    /// `asTarget` is the caller's intent, and only the caller knows it: Enter on
    /// a row runs that block's `open`, while a `then` target that produces rows
    /// is a level to descend into rather than something to run.
    nonisolated func performBlock(
        blockID: String,
        rowID: String = "",
        rowTitle: String = "",
        rowPath: String = "",
        query: String = "",
        ancestorsJSON: String = "[]",
        asTarget: Bool = false
    ) -> PerformBlockOutcome {
        let ptr = blockID.withCString { block in
            rowID.withCString { id in
                rowTitle.withCString { title in
                    rowPath.withCString { path in
                        query.withCString { query in
                            ancestorsJSON.withCString { ancestors in
                                look_perform_block_json(
                                    block, id, title, path, query, ancestors, asTarget)
                            }
                        }
                    }
                }
            }
        }
        // A failure carries a reason. An empty `errors` with nothing performed
        // means something else entirely (a block that produces rows), and the
        // caller keys on that, so the two must not share a value.
        guard let ptr else {
            return PerformBlockOutcome(
                performed: 0, errors: ["the core did not answer"], producesRows: false,
                opensPath: false)
        }
        defer { look_free_cstring(ptr) }
        // The core sends snake_case, so `produces_rows` only reaches
        // `producesRows` with the conversion strategy set.
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        guard let data = String(cString: ptr).data(using: .utf8),
              let outcome = try? decoder.decode(PerformBlockOutcome.self, from: data)
        else {
            return PerformBlockOutcome(
                performed: 0, errors: ["the core's answer could not be read"],
                producesRows: false, opensPath: false)
        }
        return outcome
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

    /// One remembered clip, as stored in the shared look.db.
    nonisolated struct ClipboardEntry: Decodable, Identifiable, Equatable {
        let id: Int64
        let content: String
        let kind: String
        let appBundleID: String?
        let copiedAtUnixS: Int64

        var copiedAt: Date { Date(timeIntervalSince1970: TimeInterval(copiedAtUnixS)) }
    }

    /// Remembers a clip and returns its row id, or nil when nothing was
    /// stored. The id is the handle a later delete needs: without it, deleting
    /// the clip would only drop the in-memory copy and it would return on the
    /// next launch.
    ///
    /// NEVER call this for a concealed or transient clip: the core cannot see
    /// pasteboard type markers, so this side is the only place a password
    /// manager's clip can be kept out of the database.
    /// Opens the shared look.db - call off the main thread.
    @discardableResult
    nonisolated func recordClipboard(content: String, kind: String = "text", appBundleID: String? = nil) -> Int64? {
        let id = content.withCString { contentC in
            kind.withCString { kindC in
                if let appBundleID {
                    return appBundleID.withCString { appC in
                        look_clipboard_record(contentC, kindC, appC)
                    }
                }
                return look_clipboard_record(contentC, kindC, nil)
            }
        }
        return id > 0 ? id : nil
    }

    /// Up to `limit` remembered clips matching `query` (newest first). An empty
    /// query returns the most recent. Opens look.db - call off the main thread.
    nonisolated func clipboardEntries(query: String = "", limit: Int) -> [ClipboardEntry] {
        let ptr = query.withCString { look_clipboard_list_json($0, UInt32(limit)) }
        guard let ptr else { return [] }
        defer { look_free_cstring(ptr) }
        guard let data = String(cString: ptr).data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([ClipboardEntry].self, from: data)) ?? []
    }

    @discardableResult
    nonisolated func deleteClipboardEntry(id: Int64) -> Bool {
        look_clipboard_delete(id)
    }

    /// Forgets every clip, returning how many were removed.
    @discardableResult
    nonisolated func clearClipboardHistory() -> Int {
        Int(look_clipboard_clear())
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

/// A user-declared block and the steps performing it will run, so the panel can
/// show exactly what Enter is about to do.
/// One preferred-tool action resolved against a row. `kind` says which of the
/// other fields are filled.
nonisolated struct ToolAction: Decodable {
    enum Kind: String, Decodable {
        /// Composed shell text. Only `look_tool_action_json` returns this;
        /// performing runs it in core and reports `performed` or `failed`.
        case shell
        /// The native side launches `tool` with `path`.
        case application
        /// Nothing declared, so the platform's own handler does it.
        case systemDefault = "system_default"
        /// Core spawned it.
        case performed
        case failed
        /// No tool declared, or one that cannot do this.
        case unavailable
    }

    let kind: Kind
    let tool: String?
    let command: String?
    let path: String?
    /// Shown as-is when `kind` is `unavailable` or `failed`.
    let reason: String?
    /// The config key that would fix an `unavailable` action.
    let key: String?
    /// A block declared this action for its own rows, so `tool` names the
    /// block: "Open in Projects" is not a label worth showing.
    let fromBlock: Bool
}

nonisolated struct SourceBlock: Decodable {
    let id: String
    let name: String
    let steps: [String]
    /// The `.toml` (or script) that declared it, for showing and revealing.
    let file: String?
    /// Where a row of this block can go next.
    let then: [SourceBlockTarget]
}

/// One `then` target. `performs` says what the target's own producer decided:
/// steps to run now, or rows to descend into.
nonisolated struct SourceBlockTarget: Decodable, Identifiable {
    let id: String
    let name: String
    let icon: String?
    let performs: Bool
    /// Already expanded against the row, so it names what will actually happen.
    let confirm: String?
}

/// A declared block as the row layer needs it: what to call it and what icon it
/// asked for. Cached per launcher open so rendering a row never touches disk.
nonisolated struct SourceBlockSummary: Decodable {
    let id: String
    let name: String
    let icon: String?
}

/// How a `run`-block refresh went. A block that failed kept the rows it had.
nonisolated struct RunBlockRefreshOutcome: Decodable {
    let refreshed: Int
    let errors: [String]
}

/// A block's `preview` output, or why it could not run.
nonisolated struct SourcePreview: Decodable {
    let text: String
    let error: String?
}

/// How performing a block went. `errors` is empty when every step was spawned;
/// a step's own exit code is its business, since nothing waits for it.
nonisolated struct PerformBlockOutcome: Decodable {
    let performed: Int
    let errors: [String]
    /// The target lists rows to pick from rather than steps to run. An explicit
    /// flag, because "nothing performed" is also what a failure looks like.
    let producesRows: Bool
    /// Nothing declared and the row has a path, so it opens like any file. The
    /// core decides this, not the row's kind.
    let opensPath: Bool
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
    /// File recall only: which fallback produced the results (see
    /// EngineBridge.FileRecallOutcome).
    let relaxed: String?
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
    let icon: String?
}

extension LauncherResult {
    /// One decode of a search payload row. Spelled out per call site, every new
    /// field the core adds is a three-site edit, and the sites drift: `icon`
    /// reached search results and missed file recall entirely.
    fileprivate nonisolated init(_ item: SearchItem, defaultKind: LauncherResultKind) {
        self.init(
            id: item.id,
            kind: LauncherResultKind(rawValue: item.kind) ?? defaultKind,
            title: item.title,
            subtitle: item.subtitle,
            path: item.path,
            score: item.score,
            icon: item.icon
        )
    }
}
