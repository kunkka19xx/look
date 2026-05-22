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

    private let cache: NSCache<NSString, NSImage> = {
        let c = NSCache<NSString, NSImage>()
        c.countLimit = 64
        return c
    }()

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

        let image: NSImage? = await withCheckedContinuation { continuation in
            QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { rep, _ in
                continuation.resume(returning: rep?.nsImage)
            }
        }

        if let image { cache.setObject(image, forKey: key) }
        return image
    }
}
