import Foundation

/// LRU cache for highlighted file previews, keyed by
/// `(path, mtime, size)` so a file is re-tokenized only when it
/// actually changes on disk.
nonisolated enum HighlightedTextCache {
    // NSCache is documented thread-safe; mark nonisolated(unsafe) to
    // satisfy Swift 6 strict-concurrency without an actor wrapper.
    /// Attribute runs (one per highlighted token) dominate the real footprint,
    /// so cost is estimated per character rather than counted per entry.
    private static let cacheCountLimit = 32
    private static let cacheCostLimitBytes = 16 * 1024 * 1024
    private static let estimatedBytesPerCharacter = 10

    nonisolated(unsafe) private static let cache: NSCache<NSString, NSAttributedString> = {
        let c = NSCache<NSString, NSAttributedString>()
        c.countLimit = cacheCountLimit
        c.totalCostLimit = cacheCostLimitBytes
        return c
    }()

    static func key(path: String, mtime: Date, size: Int64) -> String {
        // Use full sub-second precision; truncating to Int seconds
        // collides on rapid save/resave + same byte size.
        "\(path)|\(mtime.timeIntervalSince1970)|\(size)"
    }

    static func get(_ key: String) -> NSAttributedString? {
        cache.object(forKey: key as NSString)
    }

    static func set(_ key: String, _ value: NSAttributedString) {
        cache.setObject(
            value,
            forKey: key as NSString,
            cost: value.length * estimatedBytesPerCharacter
        )
    }

    /// Drops every cached preview. Called when the launcher hides so an idle
    /// Look does not keep highlighted file contents resident.
    static func purge() {
        cache.removeAllObjects()
    }
}
