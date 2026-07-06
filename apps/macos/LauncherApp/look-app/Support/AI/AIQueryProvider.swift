import Foundation

/// The kind of result an AI provider believes the user is looking for. Mirrors
/// the engine's prefix grammar (`core/engine/src/query.rs`) so an intent can be
/// rendered straight into a query string the Rust engine already understands.
enum AISearchKind: String, Codable, Sendable {
    case app
    case file
    case folder
    case recent
    case any
}

/// A provider-agnostic understanding of a natural-language query. Whatever model
/// produced it (Apple Intelligence today, a cloud LLM tomorrow), the rest of the
/// app only ever sees this struct.
struct AISearchIntent: Sendable, Equatable {
    var kind: AISearchKind
    /// The cleaned search text, with natural-language filler removed
    /// (e.g. "open my budget spreadsheet" -> "budget spreadsheet").
    var searchText: String

    /// Renders the intent into the engine's prefix grammar. Returns plain text
    /// for `.any` so the engine's default ranking applies.
    func engineQuery() -> String {
        let text = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return "" }
        switch kind {
        case .app: return "a\"\(text)"
        case .file: return "f\"\(text)"
        case .folder: return "d\"\(text)"
        case .recent: return "rc\"\(text)"
        case .any: return text
        }
    }
}

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

    /// Translate a natural-language query into a structured intent. Returns
    /// `nil` when the provider declines or fails, so the caller can fall back to
    /// the raw query - AI must never block a search.
    func understand(query: String) async -> AISearchIntent?

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
