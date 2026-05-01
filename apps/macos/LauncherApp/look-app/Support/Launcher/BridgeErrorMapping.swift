enum BridgeErrorCode: String {
    case emptyText = "empty_text"
    case invalidInput = "invalid_input"
    case invalidCandidateID = "invalid_candidate_id"
    case invalidUsageAction = "invalid_usage_action"
    case storageOpenFailed = "storage_open_failed"
    case recordUsageFailed = "record_usage_failed"
    case ffiNullResponse = "ffi_null_response"
    case decodeFailed = "decode_failed"
    case invalidTargetLang = "invalid_target_lang"
    case translateRequestFailed = "translate_request_failed"
    case translateExecFailed = "translate_exec_failed"
    case translateParseFailed = "translate_parse_failed"
    case translateDecodeFailed = "translate_decode_failed"
    case translateEmptyResult = "translate_empty_result"
    case serializeFailed = "serialize_failed"
}

nonisolated enum BridgeErrorMapping {
    static func userFacingMessage(code: String, fallback: String) -> String {
        guard let code = BridgeErrorCode(rawValue: code) else {
            return fallback
        }

        switch code {
        case .emptyText:
            return "Type some text to continue."
        case .invalidInput, .invalidCandidateID, .invalidUsageAction:
            return "This item could not be tracked."
        case .storageOpenFailed, .recordUsageFailed:
            return "Usage tracking is temporarily unavailable."
        case .ffiNullResponse, .decodeFailed:
            return "Backend response was invalid."
        case .invalidTargetLang:
            return "Language selection is not supported."
        case .translateRequestFailed, .translateExecFailed:
            return "Translation service is temporarily unavailable."
        case .translateParseFailed, .translateDecodeFailed, .translateEmptyResult:
            return "Could not read translation response."
        case .serializeFailed:
            return "Backend response was invalid."
        }
    }
}
