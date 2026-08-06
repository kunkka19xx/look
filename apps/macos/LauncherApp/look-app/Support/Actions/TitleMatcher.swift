import Foundation

/// Ranks stored items against the user's phrase for the mutate tools, and
/// applies the safety gate: never mutate on an uncertain match. Tiered scoring
/// (exact > contains > word-prefix > subsequence); a single confident winner
/// plans directly, near-ties become a choice list, no match means say so.
nonisolated enum TitleMatcher {
    static func score(query: String, title: String) -> Int? {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        let t = title.lowercased()
        guard !q.isEmpty else { return nil }
        if t == q { return 1000 }
        if t.contains(q) { return 500 }
        let queryWords = q.split(separator: " ")
        let titleWords = t.split(separator: " ")
        if !queryWords.isEmpty,
           queryWords.allSatisfy({ qw in titleWords.contains { $0.hasPrefix(qw) } }) {
            return 300
        }
        var qi = q.startIndex
        for ch in t where qi < q.endIndex && ch == q[qi] {
            qi = q.index(after: qi)
        }
        return qi == q.endIndex ? 100 : nil
    }

    enum Outcome<Candidate> {
        case one(Candidate)
        case several([Candidate])
        case none
    }

    /// The gate: one confident winner (strong score AND strictly ahead), a
    /// short choice list, or nothing.
    static func resolve<Candidate>(
        _ candidates: [Candidate],
        query: String,
        title: (Candidate) -> String
    ) -> Outcome<Candidate> {
        let scored = candidates
            .compactMap { candidate in
                score(query: query, title: title(candidate)).map { (candidate, $0) }
            }
            .sorted { $0.1 > $1.1 }
        guard let top = scored.first else { return .none }
        // Auto-plan requires a STRONG score (>= word-prefix tier) AND a clear
        // lead. A lone weak subsequence hit becomes a one-item choice instead of
        // silently mutating the wrong thing.
        if top.1 >= 300, scored.count == 1 || top.1 > scored[1].1 {
            return .one(top.0)
        }
        return .several(scored.prefix(5).map { $0.0 })
    }
}

