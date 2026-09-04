import Foundation

/// Decides whether a hide outlasted `query_retention_seconds`, and so whether the
/// next open should return to the empty home state.
///
/// Pure so it can be unit tested; the config file read lives at the call site,
/// the way every other file-only key on this platform reads its value.
enum QueryRetentionPolicy {
    /// Resolves the configured timeout from already-parsed config values.
    /// Falls back to the default when the key is missing, unparseable, or too
    /// small to be useful.
    static func resolveSeconds(from values: [String: String]) -> Int {
        let fallback = AppConstants.Launcher.QueryRetention.defaultSeconds
        guard let raw = values[AppConstants.Launcher.QueryRetention.configKey],
            let parsed = Int(raw.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            return fallback
        }
        // Ahead of the minimum check, which the opt-out sentinel is below.
        if parsed == AppConstants.Launcher.QueryRetention.never {
            return parsed
        }
        return parsed >= AppConstants.Launcher.QueryRetention.minimumSeconds ? parsed : fallback
    }

    /// Wall clock rather than a monotonic clock: a Mac spends most of its hidden
    /// time asleep, and only `Date` counts that. A clock moved backwards yields a
    /// negative interval and so preserves the query, which is the safe answer.
    static func shouldClear(hiddenAt: Date?, seconds: Int, now: Date = Date()) -> Bool {
        guard seconds >= 0, let hiddenAt else { return false }
        return now.timeIntervalSince(hiddenAt) >= TimeInterval(seconds)
    }
}
