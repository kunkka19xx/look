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
            // to the raw query — AI is best-effort, never a hard dependency.
            return nil
        }
        #else
        return nil
        #endif
    }

    private static let instructions = """
        You translate a macOS launcher search into a structured plan. The user types \
        natural language; map it to what they want to find.

        Pick `kind`:
        - app: launching an application ("open spotify", "launch terminal")
        - file: a document/file ("my budget spreadsheet", "the resume pdf")
        - folder: a directory ("downloads folder", "where my projects live")
        - recent: emphasises recently used items ("the doc I opened yesterday")
        - any: unclear, or a mix — let the launcher decide

        Set `searchText` to just the keywords to match, stripped of filler words \
        like "open", "find", "my", "the". Keep it short. Do not invent terms that \
        are not implied by the query.
        """
}

#if canImport(FoundationModels)
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
