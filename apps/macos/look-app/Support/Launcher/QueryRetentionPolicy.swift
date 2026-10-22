import Foundation

/// Whether a hide outlasted `query_retention_seconds`. Pure so it can be unit
/// tested; the config read lives at the call site, like every other file-only key.
enum QueryRetentionPolicy {
    /// Falls back to the default only when the key is missing or unparseable,
    /// so any number the user writes is the number they get. Negatives all mean
    /// the opt-out.
    static func resolveSeconds(from values: [String: String]) -> Int {
        guard let raw = values[AppConstants.Launcher.QueryRetention.configKey],
            let parsed = Int(raw.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            return AppConstants.Launcher.QueryRetention.defaultSeconds
        }
        return parsed < 0 ? AppConstants.Launcher.QueryRetention.never : parsed
    }

    /// Wall clock: a Mac spends most of its hidden time asleep, and only `Date`
    /// counts that. A backwards clock yields a negative interval, keeping the query.
    static func shouldClear(hiddenAt: Date?, seconds: Int, now: Date = Date()) -> Bool {
        guard seconds >= 0, let hiddenAt else { return false }
        return now.timeIntervalSince(hiddenAt) >= TimeInterval(seconds)
    }
}
