import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

/// On-device query understanding backed by Apple Intelligence's Foundation
/// Models framework. Runs entirely on-device (no network), matching Look's
/// local-first design. Requires macOS 26+, Apple Silicon, and Apple
/// Intelligence enabled in System Settings.
struct AppleIntelligenceProvider: AIQueryProvider {
    let id = AIProviderKind.appleIntelligence.rawValue
    let displayName = "Apple Intelligence (on-device)"

    /// Runs on the Neural Engine; nothing leaves the machine.
    let isLocal = true

    var availability: AIProviderAvailability {
        #if canImport(FoundationModels)
        guard #available(macOS 26, *) else {
            return .unavailable(.requiresNewerOS)
        }
        switch SystemLanguageModel.default.availability {
        case .available:
            return .available
        case .unavailable(.deviceNotEligible):
            return .unavailable(.requiresNewerOS)
        case .unavailable(.appleIntelligenceNotEnabled):
            return .unavailable(.appleIntelligenceNotEnabled)
        case .unavailable(.modelNotReady):
            return .unavailable(.modelNotReady)
        case .unavailable(let other):
            return .unavailable(.other("\(other)"))
        @unknown default:
            return .unavailable(.other("Unknown availability state"))
        }
        #else
        return .unavailable(.requiresNewerOS)
        #endif
    }

    /// Warms up the on-device model so the first real answer doesn't pay the
    /// cold-load cost. Cheap and idempotent - safe to call repeatedly while the
    /// user types.
    func prewarm() {
        #if canImport(FoundationModels)
        // Deliberately NOT gated on availability: right after app launch the
        // framework can report unavailable until first touched, and touching it
        // here is what wakes it up.
        guard #available(macOS 26, *) else { return }
        Task { @MainActor in AppleIntelligenceWarmer.shared.prewarm() }
        #endif
    }

    func answer(query: String) -> AsyncThrowingStream<String, Error>? {
        #if canImport(FoundationModels)
        guard #available(macOS 26, *), availability.isAvailable else { return nil }

        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        return AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let session = LanguageModelSession(instructions: Self.answerInstructions)
                    // Cap the length so answers stay launcher-sized and fast.
                    let options = GenerationOptions(maximumResponseTokens: 220)
                    // Each snapshot carries the cumulative answer so far.
                    for try await snapshot in session.streamResponse(to: trimmed, options: options) {
                        if Task.isCancelled { break }
                        continuation.yield(snapshot.content)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
        #else
        return nil
        #endif
    }

    private static let answerInstructions = """
        You are a concise assistant embedded in a macOS launcher (a small \
        Spotlight-style search box). Answer the user's question directly in at \
        most 2-4 short sentences of plain text. No markdown, no headings, no \
        bullet lists, no code fences unless the answer is literally a short \
        command. If you are unsure or the question needs the web, say so in one \
        sentence rather than guessing.
        """
}

#if canImport(FoundationModels)
/// Holds one resident session so the model stays loaded between answers. Keeping
/// a live session is what actually keeps the weights warm; answers still use a
/// fresh session each time for a clean (history-free) context.
@available(macOS 26, *)
@MainActor
private final class AppleIntelligenceWarmer {
    static let shared = AppleIntelligenceWarmer()
    private var session: LanguageModelSession?

    func prewarm() {
        let warm = session ?? LanguageModelSession()
        session = warm
        warm.prewarm()
    }
}

#endif
