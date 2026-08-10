import Foundation

/// Why a provider can't run right now - surfaced to the UI so we can tell the
/// user *what* to fix (update macOS, enable Apple Intelligence, add an API key).
enum AIProviderUnavailableReason: Equatable, Sendable {
    case requiresNewerOS
    case appleIntelligenceNotEnabled
    case modelNotReady
    case missingCredentials
    case other(String)

    var userFacingMessage: String {
        switch self {
        case .requiresNewerOS:
            return "Requires macOS 26 or later."
        case .appleIntelligenceNotEnabled:
            return "Turn on Apple Intelligence in System Settings."
        case .modelNotReady:
            return "The on-device model is still downloading."
        case .missingCredentials:
            return "Add an API key for this provider."
        case .other(let message):
            return message
        }
    }
}

enum AIProviderAvailability: Equatable, Sendable {
    case available
    case unavailable(AIProviderUnavailableReason)

    var isAvailable: Bool {
        if case .available = self { return true }
        return false
    }
}

/// A pluggable source of query understanding. Add a new provider (e.g. Claude,
/// OpenAI) by conforming a type to this protocol and registering it in
/// `AIQueryRouter`. Nothing else in the app needs to change.
protocol AIQueryProvider: Sendable {
    /// Stable identifier matching an `AIProviderKind` raw value.
    var id: String { get }
    var displayName: String { get }

    /// Whether this provider can serve a request right now.
    var availability: AIProviderAvailability { get }

    /// Stream a short, free-form answer to a natural-language question. Each
    /// yielded value is the *cumulative* answer text so far (so the UI can show
    /// it typing itself out). Returns `nil` when the provider can't answer at
    /// all; the stream may otherwise finish with an error, which the caller
    /// treats as "no answer". Purely additive - never blocks search.
    func answer(query: String) -> AsyncThrowingStream<String, Error>?

    /// Optional hint that an answer may be coming soon, so the provider can warm
    /// up resources. Default is a no-op.
    func prewarm()
}

extension AIQueryProvider {
    /// Providers that only do query understanding don't have to implement
    /// free-form answering.
    func answer(query: String) -> AsyncThrowingStream<String, Error>? { nil }

    func prewarm() {}
}
