import Foundation

/// Ranking for the `ps"` finder and `/kill`, ported field-for-field from the
/// Rust reference (`apps/linows/src-tauri/src/process.rs`) so macOS ranks like
/// linows. The fuzzy name score is injected as a closure (backed by the
/// `look_fuzzy_score` FFI over `core/matching`), keeping this module pure and
/// unit-testable. The caller lowercases the query once; this lowercases each
/// candidate title before scoring.
public enum ProcessScoring {
    /// A process candidate for ranking: just the fields scoring needs. The UI
    /// layer carries icons/detail separately, keyed by `pid`.
    public struct Candidate: Equatable {
        public let name: String
        public let pid: Int32
        /// Listening TCP ports, matched against a numeric query.
        public let ports: [Int]

        public init(name: String, pid: Int32, ports: [Int]) {
            self.name = name
            self.pid = pid
            self.ports = ports
        }
    }

    /// A ranked `/kill` target: an app (nice name + icon) or a raw process.
    /// Apps rank above processes so `kill firefox` surfaces the app.
    public struct KillTarget: Equatable {
        public let name: String
        public let pid: Int32
        public let isApp: Bool

        public init(name: String, pid: Int32, isApp: Bool) {
            self.name = name
            self.pid = pid
            self.isApp = isApp
        }
    }

    // Scores for numeric-query matches, ranked above fuzzy name matches so an
    // intentional port/PID lookup wins. Exact port beats a partial digit match.
    // Mirror the Rust i64 constants (Int is 64-bit on macOS).
    public static let portExactScore = Int.max / 2
    public static let numericPartialScore = Int.max / 4
    /// PID-only matches rank below every name match (fuzzy scores are small).
    public static let pidPartialScore = Int.min / 2

    /// True when `query` is all ASCII digits (a port/PID lookup). Mirrors the
    /// Rust `chars().all(is_ascii_digit)`; an empty string is not numeric.
    public static func isNumeric(_ query: String) -> Bool {
        !query.isEmpty && query.allSatisfy { $0.isASCII && $0.isNumber }
    }

    /// The exact port a numeric query names, or nil if it overflows a u16.
    /// Mirrors Rust `trimmed.parse::<u16>().ok()`, so `99999` yields nil (and
    /// then matches nothing, rather than panicking).
    public static func exactPort(_ query: String) -> Int? {
        UInt16(query).map(Int.init)
    }

    /// Shared scoring for both process finders. A numeric query matches a
    /// listening port (exact beats partial) or the PID; every query also
    /// fuzzy-matches the name. nil when nothing matches. Pass an empty `ports`
    /// array for sources that carry no ports (e.g. app rows).
    ///
    /// `fuzzy` scores an already-lowercased title against the (already
    /// lowercased) query the caller closed over.
    public static func score(
        name: String,
        ports: [Int],
        pid: Int32,
        trimmedQuery: String,
        isNumeric: Bool,
        exactPort: Int?,
        fuzzy: (String) -> Int?
    ) -> Int? {
        if isNumeric {
            if let port = exactPort, ports.contains(port) {
                return portExactScore
            }
            if ports.contains(where: { String($0).contains(trimmedQuery) }) {
                return numericPartialScore
            }
            if String(pid).contains(trimmedQuery) {
                // The PID tier ranks below every name hit, so it only applies
                // when the name doesn't match.
                return fuzzy(name.lowercased()) ?? pidPartialScore
            }
        }
        // Name fuzzy still runs for numeric queries: a name can contain digits.
        return fuzzy(name.lowercased())
    }

    /// Order two scored hits: score descending, then name ascending
    /// (case-insensitive) - the tiebreak used everywhere in the reference.
    /// `key` is the pre-lowercased name, so the sort doesn't re-lowercase on
    /// every comparison.
    private static func hitOrder<T>(
        _ a: (score: Int, key: String, value: T),
        _ b: (score: Int, key: String, value: T)
    ) -> Bool {
        if a.score != b.score { return a.score > b.score }
        return a.key < b.key
    }

    /// Rank `ps"` rows for a non-empty query: score each candidate, drop
    /// non-matches, sort by score then name. (The empty-query case is a plain
    /// alphabetical listing handled by the caller.)
    public static func rankProcesses(
        _ candidates: [Candidate],
        query: String,
        fuzzy: (String) -> Int?
    ) -> [Candidate] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let numeric = isNumeric(trimmed)
        let port = exactPort(trimmed)

        let scored: [(score: Int, key: String, value: Candidate)] = candidates.compactMap { row in
            guard let s = score(
                name: row.name, ports: row.ports, pid: row.pid,
                trimmedQuery: trimmed, isNumeric: numeric, exactPort: port, fuzzy: fuzzy
            ) else { return nil }
            return (s, row.name.lowercased(), row)
        }
        return scored.sorted(by: hitOrder).map(\.value)
    }

    /// Rank `/kill` targets: apps first (matched on display name), then any
    /// other process (matched on process name + numeric port/PID), deduped by
    /// PID. Mirrors `rank_kill_targets`.
    ///
    /// `apps` are (display name, pid); `procs` are process candidates. `fuzzy`
    /// scores an already-lowercased title against the caller's lowered query.
    public static func rankKillTargets(
        apps: [(name: String, pid: Int32)],
        procs: [Candidate],
        query: String,
        fuzzy: (String) -> Int?
    ) -> [KillTarget] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let numeric = isNumeric(trimmed)
        let port = exactPort(trimmed)

        // Apps carry no ports, so they match by name only; a numeric port/PID
        // query resolves through the process rows below (which own the ports).
        var appPIDs = Set<Int32>()
        let appHits: [(score: Int, key: String, value: KillTarget)] = apps.compactMap { app in
            let lowerName = app.name.lowercased()
            guard let s = fuzzy(lowerName) else { return nil }
            appPIDs.insert(app.pid)
            return (s, lowerName, KillTarget(name: app.name, pid: app.pid, isApp: true))
        }
        let sortedApps = appHits.sorted(by: hitOrder)

        // Processes already shown as an app (its representative PID) are skipped.
        let procHits: [(score: Int, key: String, value: KillTarget)] = procs.compactMap { row in
            guard !appPIDs.contains(row.pid) else { return nil }
            guard let s = score(
                name: row.name, ports: row.ports, pid: row.pid,
                trimmedQuery: trimmed, isNumeric: numeric, exactPort: port, fuzzy: fuzzy
            ) else { return nil }
            return (s, row.name.lowercased(), KillTarget(name: row.name, pid: row.pid, isApp: false))
        }
        let sortedProcs = procHits.sorted(by: hitOrder)

        return sortedApps.map(\.value) + sortedProcs.map(\.value)
    }
}
