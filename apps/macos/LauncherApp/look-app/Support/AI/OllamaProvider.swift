import Foundation

/// Query understanding and free-form answers backed by a local Ollama daemon.
/// Local by default (`http://localhost:11434`), no API key, matching Look's
/// local-first design while being strong enough for the paths the on-device
/// Apple model is too weak for. Host and model come from `ThemeStore` so a
/// Settings edit takes effect without invalidating the router's cache.
struct OllamaProvider: AIQueryProvider {
    let id = AIProviderKind.ollama.rawValue
    let displayName = "Ollama (local)"

    private var config: (host: String, model: String) {
        let settings = ThemeStore.shared.settings
        return (settings.ollamaHost, settings.ollamaModel)
    }

    /// Cached health, refreshed by a throttled background probe. The protocol's
    /// `availability` is synchronous, so we cannot block on the network here.
    var availability: AIProviderAvailability {
        let (host, model) = config
        OllamaHealthCache.shared.refreshIfStale(host: host, model: model)
        return OllamaHealthCache.shared.current(host: host, model: model)
    }

    func understand(query: String) async -> AISearchIntent? {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let (host, model) = config
        guard
            let url = URL(string: host + "/api/chat"),
            let body = OllamaCodec.chatRequestBody(
                model: model, system: Self.understandInstructions, user: trimmed,
                stream: false, format: OllamaCodec.intentJSONSchema(), numPredict: 200
            )
        else { return nil }

        guard
            let (data, response) = try? await URLSession.shared.data(for: Self.jsonPost(url, body)),
            (response as? HTTPURLResponse)?.statusCode == 200,
            let plan = OllamaCodec.decodePlan(fromChatResponse: data)
        else { return nil }

        let kind = AISearchKind(rawValue: plan.kind) ?? .any
        return AISearchIntent(kind: kind, searchText: plan.searchText)
    }

    func answer(query: String) -> AsyncThrowingStream<String, Error>? {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let (host, model) = config
        guard
            let url = URL(string: host + "/api/chat"),
            let body = OllamaCodec.chatRequestBody(
                model: model, system: Self.answerInstructions, user: trimmed,
                stream: true, format: nil, numPredict: 220
            )
        else { return nil }

        return AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let (bytes, response) = try await URLSession.shared.bytes(for: Self.jsonPost(url, body))
                    guard (response as? HTTPURLResponse)?.statusCode == 200 else {
                        continuation.finish(throwing: OllamaError.badStatus)
                        return
                    }
                    // Ollama streams deltas; the contract yields cumulative text.
                    var running = ""
                    for try await line in bytes.lines {
                        if Task.isCancelled { break }
                        guard let (delta, done) = OllamaCodec.parseStreamLine(line) else { continue }
                        if !delta.isEmpty {
                            running += delta
                            continuation.yield(running)
                        }
                        if done { break }
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    func prewarm() {
        let (host, model) = config
        OllamaHealthCache.shared.refreshIfStale(host: host, model: model)
        OllamaHealthCache.shared.warmModelIfDue(host: host, model: model)
    }

    /// Loads the model into memory (no generation), so the first real request is
    /// warm. Held resident by `keep_alive`.
    nonisolated static func warm(host: String, model: String) async {
        guard
            let url = URL(string: host + "/api/generate"),
            let body = OllamaCodec.warmRequestBody(model: model)
        else { return }
        _ = try? await URLSession.shared.data(for: jsonPost(url, body))
    }

    /// One-shot `GET /api/tags` probe mapped to availability. Called only from
    /// the background refresh, never on the UI thread.
    nonisolated static func probe(host: String, model: String) async -> AIProviderAvailability {
        guard let url = URL(string: host + "/api/tags") else {
            return .unavailable(.other("Invalid Ollama host: \(host)"))
        }
        var request = URLRequest(url: url)
        request.timeoutInterval = 2
        guard
            let (data, response) = try? await URLSession.shared.data(for: request),
            (response as? HTTPURLResponse)?.statusCode == 200
        else {
            return .unavailable(.other("Ollama is not running. Start it with: ollama serve"))
        }
        switch OllamaCodec.evaluateTags(data: data, model: model) {
        case .available:
            return .available
        case .modelMissing:
            return .unavailable(.other("Model '\(model)' not found. Run: ollama pull \(model)"))
        }
    }

    /// Installed model names from `GET /api/tags`, for the Settings picker. Empty
    /// when Ollama is unreachable, so the UI falls back to manual entry.
    nonisolated static func listModels(host: String) async -> [String] {
        guard let url = URL(string: host + "/api/tags") else { return [] }
        var request = URLRequest(url: url)
        request.timeoutInterval = 2
        guard
            let (data, response) = try? await URLSession.shared.data(for: request),
            (response as? HTTPURLResponse)?.statusCode == 200
        else {
            return []
        }
        return OllamaCodec.modelNames(fromTags: data)
    }

    /// Non-streamed `/api/chat` returning the raw response data, for the action
    /// planner (which needs multi-message + repair rounds). Nil on any failure.
    nonisolated static func chatJSON(
        host: String,
        model: String,
        messages: [[String: String]],
        format: [String: Any]?
    ) async -> Data? {
        guard
            let url = URL(string: host + "/api/chat"),
            let body = OllamaCodec.chatRequestBody(
                model: model, messages: messages, stream: false,
                format: format, numPredict: 80)
        else { return nil }
        guard
            let (data, response) = try? await URLSession.shared.data(for: jsonPost(url, body)),
            (response as? HTTPURLResponse)?.statusCode == 200
        else { return nil }
        return data
    }

    private static func jsonPost(_ url: URL, _ body: Data) -> URLRequest {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.httpBody = body
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.timeoutInterval = 30
        return request
    }

    private static let answerInstructions = """
        You are a concise assistant embedded in a macOS launcher (a small \
        Spotlight-style search box). Answer the user's question directly in at \
        most 2-4 short sentences of plain text. No markdown, no headings, no \
        bullet lists, no code fences unless the answer is literally a short \
        command. If you are unsure or the question needs the web, say so in one \
        sentence rather than guessing.
        """

    private static let understandInstructions = """
        You translate a macOS launcher search into a structured plan. The user \
        types natural language; map it to what they want to find. Reply with only \
        the JSON object.

        Pick `kind`:
        - app: launching an application ("open spotify", "launch terminal")
        - file: a document/file ("my budget spreadsheet", "the resume pdf")
        - folder: a directory ("downloads folder", "where my projects live")
        - recent: emphasises recently used items ("the doc I opened yesterday")
        - any: unclear, or a mix - let the launcher decide

        Set `searchText` to just the keywords to match, stripped of filler words \
        like "open", "find", "my", "the". Keep it short. Do not invent terms that \
        are not implied by the query.
        """
}

enum OllamaError: Error {
    case badStatus
}

/// Thread-safe cache of the last Ollama health probe, refreshed off the UI
/// thread and throttled. Mirrors the `@unchecked Sendable` + `NSLock` pattern
/// used by `AIQueryRouter`.
nonisolated final class OllamaHealthCache: @unchecked Sendable {
    static let shared = OllamaHealthCache()

    private let lock = NSLock()
    private var cached: AIProviderAvailability = .unavailable(.other("Checking Ollama..."))
    private var cachedKey = ""
    private var lastRefresh = Date.distantPast
    private var inFlight = false
    private var lastWarm = Date.distantPast
    private var warmInFlight = false

    private init() {}

    private static let staleAfter: TimeInterval = 5
    // keep_alive holds the model ~30m; re-warm well within that to refresh it.
    private static let warmAfter: TimeInterval = 300

    func current(host: String, model: String) -> AIProviderAvailability {
        lock.lock()
        defer { lock.unlock() }
        guard cachedKey == key(host, model) else {
            return .unavailable(.other("Checking Ollama..."))
        }
        return cached
    }

    func refreshIfStale(host: String, model: String) {
        let wanted = key(host, model)
        lock.lock()
        let stale = cachedKey != wanted || Date().timeIntervalSince(lastRefresh) > Self.staleAfter
        guard stale, !inFlight else {
            lock.unlock()
            return
        }
        inFlight = true
        lock.unlock()

        Task.detached(priority: .utility) { [weak self] in
            let availability = await OllamaProvider.probe(host: host, model: model)
            self?.store(availability, host: host, model: model)
        }
    }

    /// Warm the model in the background, throttled, so typing a `>` query while
    /// it loads makes the first plan fast.
    func warmModelIfDue(host: String, model: String) {
        lock.lock()
        let due = Date().timeIntervalSince(lastWarm) > Self.warmAfter
        guard due, !warmInFlight else {
            lock.unlock()
            return
        }
        warmInFlight = true
        lastWarm = Date()
        lock.unlock()

        Task.detached(priority: .utility) { [weak self] in
            await OllamaProvider.warm(host: host, model: model)
            self?.clearWarmInFlight()
        }
    }

    private func clearWarmInFlight() {
        lock.lock()
        warmInFlight = false
        lock.unlock()
    }

    private func store(_ availability: AIProviderAvailability, host: String, model: String) {
        lock.lock()
        cached = availability
        cachedKey = key(host, model)
        lastRefresh = Date()
        inFlight = false
        lock.unlock()
    }

    private func key(_ host: String, _ model: String) -> String { host + "|" + model }
}
