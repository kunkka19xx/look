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

    /// Query-understanding plan the model returns as JSON (mirrors the Apple
    /// provider's `EngineQueryPlan`).
    struct Plan: Equatable {
        let kind: String
        let searchText: String
    }

    /// JSON schema handed to `/api/chat` as `format`, so the model is forced to
    /// return exactly `{ kind, searchText }`.
    static func intentJSONSchema() -> [String: Any] {
        [
            "type": "object",
            "properties": [
                "kind": [
                    "type": "string",
                    "enum": ["app", "file", "folder", "recent", "any"],
                ],
                "searchText": ["type": "string"],
            ],
            "required": ["kind", "searchText"],
        ]
    }

    /// Body for a `/api/chat` call. `format` is set for structured (understand)
    /// calls and nil for free-form (answer) calls. Returns serialized JSON.
    static func chatRequestBody(
        model: String,
        system: String,
        user: String,
        stream: Bool,
        format: [String: Any]?,
        numPredict: Int?
    ) -> Data? {
        var options: [String: Any] = ["temperature": format == nil ? 0.4 : 0]
        if let numPredict { options["num_predict"] = numPredict }
        var body: [String: Any] = [
            "model": model,
            "messages": [
                ["role": "system", "content": system],
                ["role": "user", "content": user],
            ],
            "stream": stream,
            "options": options,
        ]
        if let format { body["format"] = format }
        return try? JSONSerialization.data(withJSONObject: body)
    }

    /// Multi-message variant for the action planner (system + user + repair
    /// turns). `messages` is a list of `["role":..., "content":...]`.
    static func chatRequestBody(
        model: String,
        messages: [[String: String]],
        stream: Bool,
        format: [String: Any]?,
        numPredict: Int?
    ) -> Data? {
        var options: [String: Any] = ["temperature": 0]
        if let numPredict { options["num_predict"] = numPredict }
        var body: [String: Any] = [
            "model": model,
            "messages": messages,
            "stream": stream,
            "options": options,
            // Keep the model resident between planner calls so it stays warm.
            "keep_alive": "30m",
        ]
        if let format { body["format"] = format }
        return try? JSONSerialization.data(withJSONObject: body)
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

    /// Decodes the `message.content` JSON string of a non-streamed understand
    /// call into a `Plan`. Returns nil on any shape mismatch.
    static func decodePlan(fromChatResponse data: Data) -> Plan? {
        guard
            let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let message = root["message"] as? [String: Any],
            let content = message["content"] as? String,
            let inner = content.data(using: .utf8),
            let plan = try? JSONSerialization.jsonObject(with: inner) as? [String: Any],
            let kind = plan["kind"] as? String,
            let searchText = plan["searchText"] as? String
        else {
            return nil
        }
        return Plan(kind: kind, searchText: searchText)
    }

    /// One NDJSON line of a streamed `/api/chat` response, reduced to the delta
    /// text and the done flag. Returns nil for lines with no message content.
    static func parseStreamLine(_ line: String) -> (delta: String, done: Bool)? {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard
            !trimmed.isEmpty,
            let data = trimmed.data(using: .utf8),
            let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return nil
        }
        let done = (root["done"] as? Bool) ?? false
        let delta = (root["message"] as? [String: Any])?["content"] as? String ?? ""
        return (delta, done)
    }

    /// Ollama streams deltas, but the `answer` contract yields the cumulative
    /// text so far. Folds delta lines into successive cumulative snapshots.
    static func cumulativeSnapshots(fromStreamLines lines: [String]) -> [String] {
        var running = ""
        var snapshots: [String] = []
        for line in lines {
            guard let (delta, done) = parseStreamLine(line) else { continue }
            if !delta.isEmpty {
                running += delta
                snapshots.append(running)
            }
            if done { break }
        }
        return snapshots
    }

    /// `llama3.1` and `llama3.1:latest` refer to the same model; compare on the
    /// tag-stripped `:latest` form.
    private static func normalizeModel(_ name: String) -> String {
        let lowered = name.lowercased()
        return lowered.hasSuffix(":latest") ? String(lowered.dropLast(7)) : lowered
    }
}
