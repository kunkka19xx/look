import Foundation

/// Selects the active AI provider and turns a raw query into an engine query.
///
/// This is the one place that knows about concrete providers. To add a new one
/// (Claude, OpenAI, …): add a case to `AIProviderKind`, create a type conforming
/// to `AIQueryProvider`, and register it in `makeProvider(for:)`.
final class AIQueryRouter: @unchecked Sendable {
    static let shared = AIQueryRouter()

    private let lock = NSLock()
    private var providers: [AIProviderKind: any AIQueryProvider] = [:]

    private init() {}

    /// Returns the provider for `kind`, lazily constructing and caching it.
    func provider(for kind: AIProviderKind) -> any AIQueryProvider {
        lock.lock()
        defer { lock.unlock() }
        if let existing = providers[kind] {
            return existing
        }
        let created = Self.makeProvider(for: kind)
        providers[kind] = created
        return created
    }

    private static func makeProvider(for kind: AIProviderKind) -> any AIQueryProvider {
        switch kind {
        case .appleIntelligence:
            return AppleIntelligenceProvider()
        // Future providers slot in here, e.g.:
        // case .claude: return ClaudeProvider()
        }
    }

    /// Rewrites `query` using `kind` into the engine's prefix grammar, or returns
    /// `nil` to signal "use the raw query unchanged". Never throws - AI is
    /// best-effort and must not block search.
    func rewrite(query: String, using kind: AIProviderKind) async -> String? {
        let provider = provider(for: kind)
        guard provider.availability.isAvailable else { return nil }
        guard let intent = await provider.understand(query: query) else { return nil }
        let rewritten = intent.engineQuery()
        // Don't bother round-tripping if the model gave us nothing useful.
        guard !rewritten.isEmpty else { return nil }
        return rewritten
    }

    /// Streams a short free-form answer for `query` using `kind`, or returns
    /// `nil` when the provider is unavailable or can't answer. Never throws at
    /// the call site - failures surface as the stream finishing with an error.
    func answer(query: String, using kind: AIProviderKind) -> AsyncThrowingStream<String, Error>? {
        let provider = provider(for: kind)
        guard provider.availability.isAvailable else { return nil }
        return provider.answer(query: query)
    }

    /// Warm up the provider so the next answer is faster. Safe to call often.
    func prewarm(_ kind: AIProviderKind) {
        let provider = provider(for: kind)
        guard provider.availability.isAvailable else { return }
        provider.prewarm()
    }

    /// Current availability of `kind`, for surfacing status in Settings.
    func availability(of kind: AIProviderKind) -> AIProviderAvailability {
        provider(for: kind).availability
    }
}
