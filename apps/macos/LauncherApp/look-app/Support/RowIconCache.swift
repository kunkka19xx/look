import AppKit
import Foundation

/// Cache for result-row icons, keyed by the path they were resolved from.
///
/// `NSWorkspace.icon(forFile:)` returns a fresh `NSImage` every call, and rows
/// resolve their icon inside `body`. Uncached, any list redraw hands SwiftUI a
/// new instance per visible row, which it redraws: every icon flickers on each
/// keypress.
nonisolated enum RowIconCache {
    // NSCache is documented thread-safe; nonisolated(unsafe) satisfies Swift 6
    // strict-concurrency without an actor wrapper, as in `HighlightedTextCache`.
    nonisolated(unsafe) private static let cache: NSCache<NSString, NSImage> = {
        let c = NSCache<NSString, NSImage>()
        // Several screenfuls, so scrolling does not evict icons about to return.
        c.countLimit = 256
        return c
    }()

    static func icon(forFile path: String) -> NSImage {
        image(key: path) {
            NSWorkspace.shared.icon(forFile: path)
        }
    }

    /// The picture itself, for a row that IS one (a copied image). Cached like
    /// every other row icon, and small on disk already, so the list draws it
    /// without decoding the full-size original.
    static func thumbnail(forFile path: String) -> NSImage? {
        if let cached = cache.object(forKey: path as NSString) {
            return cached
        }
        guard let image = NSImage(contentsOfFile: path) else { return nil }
        cache.setObject(image, forKey: path as NSString)
        return image
    }

    /// For rows with no path of their own (synthetic rows, system symbols).
    static func image(key: String, resolve: () -> NSImage) -> NSImage {
        if let cached = cache.object(forKey: key as NSString) {
            return cached
        }
        let resolved = resolve()
        cache.setObject(resolved, forKey: key as NSString)
        return resolved
    }
}
