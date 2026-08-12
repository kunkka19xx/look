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
        case .ollama:
            return OllamaProvider()
        }
    }

    /// Whether private context (calendar, clipboard, remembered facts) may be
    /// attached to a prompt for `kind`. True when the provider runs on this
    /// machine, or when the user has explicitly allowed remote context. The
    /// invariant lives here rather than in a comment, so a provider added
    /// later cannot inherit personal data by default.
    func allowsPrivateContext(_ kind: AIProviderKind) -> Bool {
        if ThemeStore.shared.settings.aiAllowRemoteContext { return true }
        return provider(for: kind).isLocal
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
    /// Not gated on availability: providers self-guard, and touching a provider
    /// that reports transiently-unavailable is exactly what wakes it up.
    func prewarm(_ kind: AIProviderKind) {
        provider(for: kind).prewarm()
    }

    /// Current availability of `kind`, for surfacing status in Settings.
    func availability(of kind: AIProviderKind) -> AIProviderAvailability {
        provider(for: kind).availability
    }
}
