import AppKit
import Foundation

/// Cache for result-row icons, keyed by the path they were resolved from.
///
/// `NSWorkspace.icon(forFile:)` returns a fresh `NSImage` on every call, and the
/// row resolves its icon inside `body`. Without a cache, any redraw of the list
/// (selecting a different row redraws all of them) hands SwiftUI a new image
/// instance per visible row, which it treats as a changed image and redraws:
/// every icon in the list visibly flickers on each keypress. Returning the same
/// instance for the same path keeps them still.
nonisolated enum RowIconCache {
    // NSCache is documented thread-safe; mark nonisolated(unsafe) to satisfy
    // Swift 6 strict-concurrency without an actor wrapper, matching
    // `HighlightedTextCache`.
    nonisolated(unsafe) private static let cache: NSCache<NSString, NSImage> = {
        let c = NSCache<NSString, NSImage>()
        // Comfortably more than a screenful of rows, so scrolling a long result
        // list does not evict icons that are about to come back into view.
        c.countLimit = 256
        return c
    }()

    /// The icon for `path`, resolving and caching it on first use.
    static func icon(forFile path: String) -> NSImage {
        image(key: path) {
            NSWorkspace.shared.icon(forFile: path)
        }
    }

    /// The icon for a row that has no path of its own (synthetic rows, system
    /// symbols), cached under a caller-supplied key.
    static func image(key: String, resolve: () -> NSImage) -> NSImage {
        if let cached = cache.object(forKey: key as NSString) {
            return cached
        }
        let resolved = resolve()
        cache.setObject(resolved, forKey: key as NSString)
        return resolved
    }
}
