import AppKit
import Foundation
import QuickLookThumbnailing

/// Generates Quick Look thumbnails for file results, with a size gate
/// (so we don't ask the OS to syntax-highlight a 20 MB log file just to
/// render a tile) and an LRU cache keyed by (path, mtime, size).
///
/// See specs/quicklook-file-preview.md for the full behavior contract.
actor QuickLookPreviewService {
    static let shared = QuickLookPreviewService()

    /// Decompressed retina thumbnails run several MB each, so the cache is
    /// bounded by estimated bitmap bytes, not just entry count.
    private static let cacheCountLimit = 64
    private static let cacheCostLimitBytes = 48 * 1024 * 1024
    private static let bytesPerPixel = 4

    private let cache: NSCache<NSString, NSImage> = {
        let c = NSCache<NSString, NSImage>()
        c.countLimit = cacheCountLimit
        c.totalCostLimit = cacheCostLimitBytes
        return c
    }()

    private static func estimatedBitmapBytes(_ image: NSImage) -> Int {
        let rep = image.representations.first
        let width = rep?.pixelsWide ?? Int(image.size.width)
        let height = rep?.pixelsHigh ?? Int(image.size.height)
        return width * height * bytesPerPixel
    }

    /// Advanced on purge so a render in flight across the await does not
    /// repopulate the cache after it was emptied.
    private var purgeGeneration = 0

    /// Drops every cached thumbnail. Called when the launcher hides so an
    /// idle Look does not keep megabytes of preview bitmaps resident.
    func purge() {
        purgeGeneration += 1
        cache.removeAllObjects()
    }

    // Text/code files: QuickLook renders the *whole* file to produce a
    // thumbnail. Cap aggressively so 5 MB JSON dumps don't stall.
    private static let textExtensions: Set<String> = [
        "txt", "md", "markdown", "rst", "log", "csv", "tsv",
        "json", "yaml", "yml", "toml", "ini", "conf", "cfg", "env",
        "xml", "html", "htm", "css", "scss", "sass", "less",
        "js", "mjs", "cjs", "ts", "tsx", "jsx",
        "py", "rb", "go", "rs", "swift",
        "c", "cc", "cpp", "cxx", "h", "hh", "hpp", "hxx", "m", "mm",
        "java", "kt", "kts", "scala", "groovy",
        "sh", "bash", "zsh", "fish",
        "sql", "lua", "php", "pl", "r", "clj", "ex", "exs", "erl",
        "hs", "ml", "fs", "fsx", "dart", "vue", "svelte"
    ]

    private static let textFileSizeCap: Int64 = 512 * 1024            // 512 KB
    private static let defaultSizeCap: Int64 = 20 * 1024 * 1024       // 20 MB

    static func sizeCap(forPath path: String) -> Int64 {
        return isTextFile(path: path) ? textFileSizeCap : defaultSizeCap
    }

    /// What a preview reads at most. 64 KB is around 2000 lines, well past
    /// what any pane shows, and both text previews stop here.
    nonisolated static let headByteCap = 64 * 1024

    /// The head of a text file, memory-mapped and capped. The decode is lossy
    /// on purpose: it never fails on a partial codepoint at the cap or on
    /// invalid bytes, where a strict decode would show nothing at all.
    nonisolated static func head(ofFile path: String, bytes: Int = headByteCap) -> String? {
        guard
            let data = try? Data(contentsOf: URL(fileURLWithPath: path), options: [.mappedIfSafe])
        else { return nil }
        return String(decoding: data.prefix(bytes), as: UTF8.self)
    }

    /// Markdown gets its own preview. `rst` stays text: it is in the list
    /// above but is a different syntax, and our parser would mangle it.
    static func isMarkdownFile(path: String) -> Bool {
        ["md", "markdown"].contains((path as NSString).pathExtension.lowercased())
    }

    static func isTextFile(path: String) -> Bool {
        let ext = (path as NSString).pathExtension.lowercased()
        return textExtensions.contains(ext)
    }

    /// Returns nil if the file is missing, exceeds the size cap for its
    /// type, or QuickLook produced no representation.
    func thumbnail(forPath path: String, size: CGSize, scale: CGFloat) async -> NSImage? {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: path),
              let fileSize = attrs[.size] as? Int64,
              let mtime = attrs[.modificationDate] as? Date else {
            return nil
        }
        guard fileSize <= Self.sizeCap(forPath: path) else { return nil }

        // Full sub-second mtime precision: truncating to Int seconds
        // collides on rapid save/resave + same byte size.
        let key = "\(path)|\(mtime.timeIntervalSince1970)|\(fileSize)|\(Int(size.width))x\(Int(size.height))@\(scale)" as NSString
        if let cached = cache.object(forKey: key) { return cached }

        let request = QLThumbnailGenerator.Request(
            fileAt: URL(fileURLWithPath: path),
            size: size,
            scale: scale,
            representationTypes: .thumbnail
        )

        let generation = purgeGeneration
        let image: NSImage? = await withCheckedContinuation { continuation in
            QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { rep, _ in
                continuation.resume(returning: rep?.nsImage)
            }
        }

        if let image, generation == purgeGeneration {
            cache.setObject(image, forKey: key, cost: Self.estimatedBitmapBytes(image))
        }
        return image
    }
}
