import Foundation

/// Whether a hide outlasted `query_retention_seconds`. Pure so it can be unit
/// tested; the config read lives at the call site, like every other file-only key.
enum QueryRetentionPolicy {
    /// Falls back to the default when the key is missing, unparseable, or too
    /// small to be useful.
    static func resolveSeconds(from values: [String: String]) -> Int {
        let fallback = AppConstants.Launcher.QueryRetention.defaultSeconds
        guard let raw = values[AppConstants.Launcher.QueryRetention.configKey],
            let parsed = Int(raw.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            return fallback
        }
        // Ahead of the minimum check, which the sentinel is below.
        if parsed == AppConstants.Launcher.QueryRetention.never {
            return parsed
        }
        return parsed >= AppConstants.Launcher.QueryRetention.minimumSeconds ? parsed : fallback
    }

    /// Wall clock: a Mac spends most of its hidden time asleep, and only `Date`
    /// counts that. A backwards clock yields a negative interval, keeping the query.
    static func shouldClear(hiddenAt: Date?, seconds: Int, now: Date = Date()) -> Bool {
        guard seconds >= 0, let hiddenAt else { return false }
        return now.timeIntervalSince(hiddenAt) >= TimeInterval(seconds)
    }
}
