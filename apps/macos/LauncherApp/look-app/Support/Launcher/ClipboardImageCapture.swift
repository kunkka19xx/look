import AppKit
import CryptoKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// One copied image, as the `ci"` list shows it.
///
/// The pixels are not here: they live in a file named after `hash`, because a
/// screenshot is megabytes and the history is a list the user scrolls.
nonisolated struct ClipboardImageEntry: Identifiable, Equatable {
    /// What the row is titled and searched by. A filename when the image was
    /// copied as a file, else where it came from and when, since raw pasteboard
    /// pixels arrive with no name at all.
    let label: String
    /// Identity of the pixels, and so of the row: it names the files and is
    /// what the store dedupes on, which is why a re-copy keeps the row it
    /// already had instead of being given a fresh one.
    let hash: String
    let capturedAt: Date
    let pixelSize: CGSize
    let byteSize: Int
    let appBundleID: String?
    /// Row id in the persisted history. Without it a delete here would leave
    /// the image on disk and it would return on the next launch.
    let storeID: Int64?

    var id: String { hash }
    var fileURL: URL? { ClipboardImageFiles.fileURL(forHash: hash) }
    var thumbnailURL: URL? { ClipboardImageFiles.thumbnailURL(forHash: hash) }
}

/// What a freshly stored image turned out to be, once its bytes were normalized
/// and written.
nonisolated struct StoredClipboardImage {
    let hash: String
    let pixelSize: CGSize
    let byteSize: Int
}

/// The pixels an image clip is made of: where they live, and what reading or
/// writing them costs.
///
/// Every call here is off the main thread's path - encoding and hashing a
/// screenshot takes long enough to drop frames while typing.
nonisolated enum ClipboardImageFiles {
    private typealias Constants = AppConstants.Launcher.ClipboardImage

    /// Core owns the location, creates it, and sweeps anything in it that no
    /// row claims, so this side asks rather than assembling a path.
    private static let directory: URL? = EngineBridge.shared.clipboardImagesDirectory()

    static func fileURL(forHash hash: String) -> URL? {
        url(forHash: hash, thumbnail: false)
    }

    static func thumbnailURL(forHash hash: String) -> URL? {
        url(forHash: hash, thumbnail: true)
    }

    /// Normalizes `data` to PNG, writes it and its thumbnail under the hash of
    /// the normalized bytes, and reports what was stored. Re-copying the same
    /// image finds both files already there and rewrites nothing.
    static func store(_ data: Data) -> StoredClipboardImage? {
        guard let png = pngData(from: data), let pixelSize = pixelSize(of: png) else {
            return nil
        }
        let hash = SHA256.hash(data: png).map { String(format: "%02x", $0) }.joined()
        guard let fileURL = fileURL(forHash: hash), let thumbnailURL = thumbnailURL(forHash: hash)
        else {
            return nil
        }
        if !FileManager.default.fileExists(atPath: fileURL.path) {
            guard (try? png.write(to: fileURL, options: .atomic)) != nil else { return nil }
        }
        if !FileManager.default.fileExists(atPath: thumbnailURL.path),
            let thumbnail = thumbnailData(from: png)
        {
            try? thumbnail.write(to: thumbnailURL, options: .atomic)
        }
        return StoredClipboardImage(hash: hash, pixelSize: pixelSize, byteSize: png.count)
    }

    /// Core sweeps the directory by hash prefix, so every name it finds has to
    /// start with the hash of the row that owns it.
    private static func url(forHash hash: String, thumbnail: Bool) -> URL? {
        guard !hash.isEmpty, let directory else { return nil }
        let suffix = thumbnail ? Constants.thumbnailSuffix : ""
        return directory.appendingPathComponent("\(hash)\(suffix).\(Constants.fileExtension)")
    }

    /// One encoding for every clip, so the same picture pasted as PNG and as
    /// TIFF is one row rather than two. Already-PNG bytes are kept verbatim:
    /// re-encoding them would only cost time.
    private static func pngData(from data: Data) -> Data? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        if let type = CGImageSourceGetType(source), (type as String) == UTType.png.identifier {
            return data
        }
        guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        return encodePNG(image)
    }

    /// Reads the dimensions out of a stored image's header, without decoding
    /// the pixels. Used when history is restored at launch.
    static func pixelSize(ofFileAt url: URL) -> CGSize? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        return pixelSize(in: source)
    }

    private static func pixelSize(of data: Data) -> CGSize? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        return pixelSize(in: source)
    }

    private static func pixelSize(in source: CGImageSource) -> CGSize? {
        guard
            let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil)
                as? [CFString: Any],
            let width = properties[kCGImagePropertyPixelWidth] as? Int,
            let height = properties[kCGImagePropertyPixelHeight] as? Int
        else {
            return nil
        }
        return CGSize(width: width, height: height)
    }

    /// A row draws this, not the full image: decoding a multi-megabyte PNG
    /// inside a list cell stutters every keystroke.
    private static func thumbnailData(from data: Data) -> Data? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: Constants.thumbnailMaxPixel,
        ]
        guard
            let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
        else {
            return nil
        }
        return encodePNG(thumbnail)
    }

    private static func encodePNG(_ image: CGImage) -> Data? {
        let output = NSMutableData()
        guard
            let destination = CGImageDestinationCreateWithData(
                output, UTType.png.identifier as CFString, 1, nil)
        else {
            return nil
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return output as Data
    }
}

/// What the pasteboard is offering, when it is offering an image.
nonisolated struct ClipboardImageCandidate {
    let data: Data
    /// The filename the image arrived under, when it arrived as a file. Raw
    /// pixels have none, and get a label built from their source instead.
    let fileName: String?
}

/// Reading images off the pasteboard. Separate from the store so the polling
/// loop stays one method: this decides only whether a copy was an image.
enum ClipboardImagePasteboard {
    private typealias Constants = AppConstants.Launcher.ClipboardImage

    /// The image on the pasteboard, or nil when there is none worth keeping.
    ///
    /// Files are checked first because an image copied in Finder carries both a
    /// file URL and a preview, and the filename is the better label.
    static func candidate(
        from pasteboard: NSPasteboard, types: [NSPasteboard.PasteboardType]
    ) -> ClipboardImageCandidate? {
        fileCandidate(from: pasteboard) ?? rawCandidate(from: pasteboard, types: types)
    }

    /// A label for pixels that arrived without a filename: the app they came
    /// from, and when, so a list of them can still be told apart.
    static func label(forSourceNamed sourceName: String?, capturedAt: Date) -> String {
        let source = sourceName ?? Constants.unnamedLabelFallbackSource
        let time = labelDateFormatter.string(from: capturedAt)
        return "\(Constants.unnamedLabelPrefix) \(source), \(time)"
    }

    private static let labelDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()

    private static func fileCandidate(from pasteboard: NSPasteboard) -> ClipboardImageCandidate? {
        let urls =
            pasteboard.readObjects(
                forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        // Size is checked from the directory entry, before the read: this runs
        // on the main thread, and pulling a print-resolution scan into memory
        // to find out it was too big would freeze the launcher.
        guard
            let url = urls.first(where: { url in
                guard
                    let values = try? url.resourceValues(forKeys: [.contentTypeKey, .fileSizeKey]),
                    values.contentType?.conforms(to: .image) == true
                else {
                    return false
                }
                return (values.fileSize ?? 0) <= Constants.maxImageBytes
            }), let data = try? Data(contentsOf: url)
        else {
            return nil
        }
        return ClipboardImageCandidate(data: data, fileName: url.lastPathComponent)
    }

    private static func rawCandidate(
        from pasteboard: NSPasteboard, types: [NSPasteboard.PasteboardType]
    ) -> ClipboardImageCandidate? {
        for type in [NSPasteboard.PasteboardType.png, .tiff] where types.contains(type) {
            if let data = pasteboard.data(forType: type) {
                return ClipboardImageCandidate(data: data, fileName: nil)
            }
        }
        return nil
    }
}
