import Combine
import Foundation

/// Drives the inline "AI answer" card shown at the top of launcher results.
///
/// It is intentionally additive: it only ever fires for *question-like* queries
/// and never blocks search. Results paint from `LauncherView+Search`; this just
/// streams a short on-device answer alongside them.
@MainActor
final class AIAnswerController: ObservableObject {
    enum State: Equatable {
        case idle       // not a question / AI off - no card
        case streaming  // answer is being generated
        case done       // answer complete
        case failed     // model declined or errored
    }

    /// One source's answer block. Identity is the source name so SwiftUI keeps
    /// each block stable as others arrive.
    struct Item: Identifiable, Equatable {
        var id: String { source }
        let text: String
        let source: String
        let url: URL?
        let imageURL: URL?
    }

    @Published private(set) var question: String = ""
    /// Finished blocks (Calculator / DuckDuckGo / Wikipedia), in arrival order.
    @Published private(set) var items: [Item] = []
    /// Streaming on-device model answer, used only when no web/calc source hit.
    @Published private(set) var llmAnswer: String = ""
    @Published private(set) var state: State = .idle

    /// Whether the card should be shown at all.
    var isActive: Bool { state != .idle }

    private var task: Task<Void, Never>?
    private let router: AIQueryRouter

    /// Wait for typing to settle before spending a model generation. The model
    /// is prewarmed as the user types, so this can stay short.
    private static let debounceNanoseconds: UInt64 = 350_000_000

    init(router: AIQueryRouter = .shared) {
        self.router = router
    }

    /// Re-evaluate for the current query. Cancels any in-flight generation.
    /// `resultCount` is how many local results the launcher found - an entity
    /// with no local match (e.g. "david beckham") is treated as knowledge-seeking.
    func update(query: String, resultCount: Int, aiEnabled: Bool, provider: AIProviderKind) {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)

        // Warm the model the moment a query starts looking sentence-like (more
        // than one word), so it's loaded by the time the user stops typing.
        if aiEnabled, trimmed.contains(" ") {
            router.prewarm(provider)
        }

        // Triggers: an explicit question, a multi-word entity that matched
        // nothing locally, or a pattern-gated instant source (weather, currency,
        // crypto) - those carry their own narrow grammar.
        let questionLike = Self.isQuestionLike(trimmed)
        let orphanEntity = resultCount == 0 && Self.isEntityLookup(trimmed)
        let instant = EngineBridge.shared.instantAnswerMatches(trimmed)
        guard aiEnabled, questionLike || orphanEntity || instant else {
            cancel()
            return
        }

        // Same question already answered/answering - leave it be.
        if trimmed == question, state != .idle { return }

        task?.cancel()
        question = trimmed
        items = []
        llmAnswer = ""
        state = .streaming

        task = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.debounceNanoseconds)
            guard let self, !Task.isCancelled else { return }

            // Fastest path: local arithmetic. No network, no model - instant.
            if let calc = Self.calcAnswer(for: trimmed) {
                if Task.isCancelled { return }
                self.items = [Item(text: calc, source: "Calculator", url: nil, imageURL: nil)]
                self.state = .done
                return
            }

            // Web sources run concurrently and each renders the moment it lands -
            // first available first, the slower one slots in below it.
            await self.collectWebAnswers(for: trimmed, questionLike: questionLike)
            if Task.isCancelled { return }
            if !self.items.isEmpty {
                self.state = .done
                return
            }

            // Fall back to streaming the on-device model.
            guard let stream = self.router.answer(query: trimmed, using: provider) else {
                self.state = .failed
                return
            }

            do {
                for try await partial in stream {
                    if Task.isCancelled { return }
                    self.llmAnswer = partial
                }
                if !Task.isCancelled {
                    self.state = self.llmAnswer.isEmpty ? .failed : .done
                }
            } catch is CancellationError {
                // Superseded by a newer query - nothing to surface.
            } catch {
                if !Task.isCancelled { self.state = .failed }
            }
        }
    }

    /// Runs the web sources concurrently and appends each result as it arrives,
    /// skipping duplicates (same source, or near-identical text).
    private func collectWebAnswers(for query: String, questionLike: Bool) async {
        // Choose what (if anything) to search Wikipedia for:
        // - "what is X" -> the entity X
        // - a bare entity ("david beckham") -> the query itself
        // - a how-to/why question -> skip; Wikipedia would mislead ("Vim is…")
        let wikiTerm: String?
        if let entity = EngineBridge.shared.definitionalEntity(query: query) {
            wikiTerm = entity
        } else if !questionLike {
            wikiTerm = query
        } else {
            wikiTerm = nil
        }

        await withTaskGroup(of: WebAnswer?.self) { group in
            // A matched instant source (weather/currency/crypto) is what the user
            // wants - skip the generic encyclopedia lookups then. The match check
            // is network-free; the answer fetch runs in the Rust core off-thread.
            let bridge = EngineBridge.shared
            if bridge.instantAnswerMatches(query) {
                group.addTask {
                    await Task.detached(priority: .userInitiated) {
                        bridge.instantAnswer(query: query)
                    }.value
                }
            } else {
                group.addTask {
                    await Task.detached(priority: .userInitiated) {
                        bridge.duckDuckGoAnswer(query: query)
                    }.value
                }
                if let wikiTerm {
                    group.addTask {
                        await Task.detached(priority: .userInitiated) {
                            bridge.wikipediaAnswer(searchTerm: wikiTerm)
                        }.value
                    }
                }
            }

            for await result in group {
                if Task.isCancelled { break }
                guard let result else { continue }
                if items.contains(where: { $0.source == result.source }) { continue }
                if items.contains(where: { Self.similar($0.text, result.text) }) { continue }
                items.append(Item(text: result.text, source: result.source, url: result.url, imageURL: result.imageURL))
                // Surfacing the first block early; more may still append.
                state = .streaming
            }
        }
    }

    /// Two extracts are "the same" if their leading text matches - DuckDuckGo
    /// abstracts are often verbatim Wikipedia, so we don't show both.
    private static func similar(_ a: String, _ b: String) -> Bool {
        func key(_ s: String) -> String {
            String(s.lowercased().filter { !$0.isWhitespace }.prefix(60))
        }
        return key(a) == key(b)
    }

    /// Tear down the card (query cleared, launcher hidden, AI turned off).
    func cancel() {
        task?.cancel()
        task = nil
        if state != .idle || !items.isEmpty || !llmAnswer.isEmpty || !question.isEmpty {
            question = ""
            items = []
            llmAnswer = ""
            state = .idle
        }
    }

    /// A cheap heuristic for "this looks like a question, not an app/file launch".
    /// Keeps the model off the hot path for ordinary launches like "spotify".
    static func isQuestionLike(_ text: String) -> Bool {
        guard text.count >= 3 else { return false }
        // An explicit question mark is the strongest signal - honour it even for
        // short queries like "1+1=?".
        if text.hasSuffix("?") { return true }

        let words = text.split(whereSeparator: { $0 == " " || $0 == "\n" })
        guard words.count >= 3 else { return false }

        let starters: Set<String> = [
            "how", "what", "why", "who", "when", "where", "which", "whose",
            "can", "could", "should", "would", "is", "are", "am", "do", "does",
            "did", "will", "explain", "tell", "give", "write", "summarize",
            "summarise", "define", "translate", "convert", "calculate",
        ]
        return starters.contains(words[0].lowercased())
    }

    /// A multi-word, reasonably long query that's likely a name/entity rather
    /// than a half-typed token. Combined with "zero local results", this marks a
    /// knowledge lookup ("david beckham") vs. an app launch ("activity monitor").
    static func isEntityLookup(_ text: String) -> Bool {
        guard text.count >= 5 else { return false }
        let words = text.split(whereSeparator: { $0 == " " || $0 == "\n" })
        return words.count >= 2
    }

    /// Evaluates a math query like "1+1=?" -> "1+1 = 2" using the launcher's own
    /// calculator. Returns nil for anything that isn't a pure arithmetic
    /// expression, so real questions fall through to web/LLM.
    private static func calcAnswer(for query: String) -> String? {
        var expr = query
        // People tack "=?", "=", or "?" onto math; strip it before evaluating.
        while let last = expr.last, "=? ".contains(last) { expr.removeLast() }
        guard !expr.isEmpty else { return nil }
        // Require an operator so a bare number or word isn't "calculated".
        guard expr.contains(where: { "+-*/^%".contains($0) }) else { return nil }
        guard CalcCommand.isReadyForEvaluation(expr) else { return nil }
        guard case .value(let result) = CalcCommand.evaluate(expr) else { return nil }
        return "\(expr) = \(result)"
    }
}
