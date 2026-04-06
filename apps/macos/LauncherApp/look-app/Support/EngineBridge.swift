import Foundation

@_silgen_name("look_search_json")
nonisolated
private func look_search_json(_ query: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_record_usage")
nonisolated
private func look_record_usage(_ candidateID: UnsafePointer<CChar>?, _ action: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_free_cstring")
nonisolated
private func look_free_cstring(_ ptr: UnsafeMutablePointer<CChar>?)

@_silgen_name("look_reload_config")
nonisolated
private func look_reload_config() -> Bool

@_silgen_name("look_translate_json")
nonisolated
private func look_translate_json(_ text: UnsafePointer<CChar>?, _ targetLang: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_clipboard_store")
nonisolated
private func look_clipboard_store(_ content: UnsafePointer<CChar>?, _ contentType: UnsafePointer<CChar>?, _ sourceApp: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_clipboard_search")
nonisolated
private func look_clipboard_search(_ query: UnsafePointer<CChar>?, _ contentType: UnsafePointer<CChar>?, _ limit: UInt32) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_clipboard_delete")
nonisolated
private func look_clipboard_delete(_ itemId: UnsafePointer<CChar>?) -> Bool

@_silgen_name("look_clipboard_clear")
nonisolated
private func look_clipboard_clear(_ olderThanSeconds: Int64) -> UInt64

@_silgen_name("look_clipboard_toggle_pin")
nonisolated
private func look_clipboard_toggle_pin(_ itemId: UnsafePointer<CChar>?) -> Bool

final class EngineBridge {
    static let shared = EngineBridge()

    private init() {}

    nonisolated func search(query: String, limit: Int = 40) -> [LauncherResult] {
        let ptr = query.withCString { cstr in
            look_search_json(cstr, UInt32(limit))
        }

        guard let ptr else {
            return fallbackResults()
        }

        defer {
            look_free_cstring(ptr)
        }

        let raw = String(cString: ptr)
        guard let data = raw.data(using: .utf8),
            let payload = try? JSONDecoder().decode(SearchPayload.self, from: data)
        else {
            return fallbackResults()
        }

        if payload.error != nil {
            return fallbackResults()
        }

        return payload.results.map {
            LauncherResult(
                id: $0.id,
                kind: LauncherResultKind(rawValue: $0.kind) ?? .app,
                title: $0.title,
                subtitle: $0.subtitle,
                path: $0.path,
                score: $0.score
            )
        }
    }

    nonisolated func recordUsage(candidateID: String, action: String) {
        _ = candidateID.withCString { idCstr in
            action.withCString { actionCstr in
                look_record_usage(idCstr, actionCstr)
            }
        }
    }

    nonisolated func reloadConfig() -> Bool {
        look_reload_config()
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

    nonisolated func storeClipboard(content: String, contentType: String, sourceApp: String) -> Bool {
        content.withCString { contentCstr in
            contentType.withCString { typeCstr in
                sourceApp.withCString { appCstr in
                    look_clipboard_store(contentCstr, typeCstr, appCstr)
                }
            }
        }
    }

    nonisolated func searchClipboard(query: String = "", contentType: String = "", limit: Int = 50) -> [ClipboardEntry] {
        let ptr = query.withCString { queryCstr in
            contentType.withCString { typeCstr in
                look_clipboard_search(queryCstr, typeCstr, UInt32(limit))
            }
        }

        guard let ptr else { return [] }
        defer { look_free_cstring(ptr) }

        let raw = String(cString: ptr)
        guard let data = raw.data(using: .utf8),
              let payload = try? JSONDecoder().decode(ClipboardSearchPayload.self, from: data)
        else { return [] }

        if payload.error != nil { return [] }
        return payload.items
    }

    nonisolated func deleteClipboardItem(id: String) -> Bool {
        id.withCString { look_clipboard_delete($0) }
    }

    nonisolated func clearClipboardHistory(olderThanSeconds: Int64 = 0) -> UInt64 {
        look_clipboard_clear(olderThanSeconds)
    }

    nonisolated func toggleClipboardPin(id: String) -> Bool {
        id.withCString { look_clipboard_toggle_pin($0) }
    }

    nonisolated private func fallbackResults() -> [LauncherResult] {
        []
    }
}

struct TranslationResult: Decodable {
    let original: String
    let translated: String
    let error: BridgeError?
}

private nonisolated struct SearchPayload: Decodable {
    let query: String
    let count: Int
    let results: [SearchItem]
    let error: BridgeError?
}

struct BridgeError: Decodable {
    let code: String
    let message: String
}

private nonisolated struct SearchItem: Decodable {
    let id: String
    let kind: String
    let title: String
    let subtitle: String?
    let path: String
    let score: Int
}

private nonisolated struct ClipboardSearchPayload: Decodable {
    let count: Int
    let items: [ClipboardEntry]
    let error: BridgeError?
}
