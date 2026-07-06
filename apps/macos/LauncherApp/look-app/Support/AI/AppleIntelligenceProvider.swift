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

    func understand(query: String) async -> AISearchIntent? {
        #if canImport(FoundationModels)
        guard #available(macOS 26, *), availability.isAvailable else { return nil }

        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        do {
            let session = LanguageModelSession(instructions: Self.instructions)
            let response = try await session.respond(
                to: trimmed,
                generating: EngineQueryPlan.self
            )
            return response.content.asIntent()
        } catch {
            // Any failure (guardrails, generation error, cancellation) falls back
            // to the raw query - AI is best-effort, never a hard dependency.
            return nil
        }
        #else
        return nil
        #endif
    }

    /// Warms up the on-device model so the first real answer doesn't pay the
    /// cold-load cost. Cheap and idempotent - safe to call repeatedly while the
    /// user types.
    func prewarm() {
        #if canImport(FoundationModels)
        guard #available(macOS 26, *), availability.isAvailable else { return }
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

    private static let instructions = """
        You translate a macOS launcher search into a structured plan. The user types \
        natural language; map it to what they want to find.

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

@available(macOS 26, *)
@Generable
private struct EngineQueryPlan {
    @Guide(description: "The kind of thing the user wants to find.")
    let kind: PlanKind

    @Guide(description: "Just the keywords to search for, with filler words removed.")
    let searchText: String

    @Generable
    enum PlanKind: String {
        case app
        case file
        case folder
        case recent
        case any
    }

    func asIntent() -> AISearchIntent {
        AISearchIntent(kind: kind.asSearchKind, searchText: searchText)
    }
}

@available(macOS 26, *)
extension EngineQueryPlan.PlanKind {
    var asSearchKind: AISearchKind {
        switch self {
        case .app: return .app
        case .file: return .file
        case .folder: return .folder
        case .recent: return .recent
        case .any: return .any
        }
    }
}
#endif
