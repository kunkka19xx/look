import Foundation

/// Pure request/response codec for the Ollama REST API. Foundation-only and free
/// of app types, so it lives in the `LauncherLogic` package and is unit-tested
/// without a live daemon. `OllamaProvider` (app target) does the networking and
/// maps these plain results onto the app's AI types.
nonisolated enum OllamaCodec {
    /// Health derived from `GET /api/tags`. Reaching the endpoint means the
    /// daemon is up; the only question left is whether the model is pulled.
    enum Health: Equatable {
        case available
        case modelMissing
    }

    /// Body for `/api/generate` that just loads (warms) the model without
    /// generating, holding it resident for `keep_alive`.
    static func warmRequestBody(model: String) -> Data? {
        try? JSONSerialization.data(withJSONObject: [
            "model": model,
            "keep_alive": "30m",
        ])
    }

    /// Parses `GET /api/tags` payload and reports whether `model` is present.
    /// Ollama tags may carry an implicit `:latest`, so match on the bare name too.
    static func evaluateTags(data: Data, model: String) -> Health {
        guard
            let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let models = root["models"] as? [[String: Any]]
        else {
            return .modelMissing
        }
        let names = models.compactMap { $0["name"] as? String }
        let wanted = normalizeModel(model)
        let present = names.contains { normalizeModel($0) == wanted }
        return present ? .available : .modelMissing
    }

    /// Names of installed models from `GET /api/tags`, sorted. Empty on any
    /// failure, so the caller can fall back to manual entry.
    static func modelNames(fromTags data: Data) -> [String] {
        guard
            let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let models = root["models"] as? [[String: Any]]
        else {
            return []
        }
        return models.compactMap { $0["name"] as? String }.sorted()
    }

    /// `llama3.1` and `llama3.1:latest` refer to the same model; compare on the
    /// tag-stripped `:latest` form.
    private static func normalizeModel(_ name: String) -> String {
        let lowered = name.lowercased()
        return lowered.hasSuffix(":latest") ? String(lowered.dropLast(7)) : lowered
    }
}
