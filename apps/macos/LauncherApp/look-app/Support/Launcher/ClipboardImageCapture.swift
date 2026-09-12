import AppKit
import CryptoKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// One copied image. The pixels are not here: they live in a file named after
/// `hash`, since a screenshot is megabytes and this is a list the user scrolls.
nonisolated struct ClipboardImageEntry: Identifiable, Equatable {
    /// A filename when the image arrived as one, else where it came from and
    /// when, since raw pasteboard pixels have no name.
    let label: String
    /// Identity of the pixels, and so of the row: a re-copy lands on the row it
    /// already had rather than a new one.
    let hash: String
    let capturedAt: Date
    let pixelSize: CGSize
    let byteSize: Int
    let appBundleID: String?
    /// Row id in the persisted history, without which a delete here would
    /// leave the image on disk to return next launch.
    let storeID: Int64?

    var id: String { hash }
    var fileURL: URL? { ClipboardImageFiles.fileURL(forHash: hash) }
    var thumbnailURL: URL? { ClipboardImageFiles.thumbnailURL(forHash: hash) }
}

nonisolated struct StoredClipboardImage {
    let hash: String
    let pixelSize: CGSize
    let byteSize: Int
}

/// Where an image clip's pixels live. Every call belongs off the main thread:
/// encoding and hashing a screenshot drops frames while typing.
nonisolated enum ClipboardImageFiles {
    private typealias Constants = AppConstants.Launcher.ClipboardImage

    /// Core owns the location and sweeps it, so this side asks rather than
    /// assembling a path.
    private static let directory: URL? = EngineBridge.shared.clipboardImagesDirectory()

    static func fileURL(forHash hash: String) -> URL? {
        url(forHash: hash, thumbnail: false)
    }

    static func thumbnailURL(forHash hash: String) -> URL? {
        url(forHash: hash, thumbnail: true)
    }

    /// Normalizes to PNG and writes the image and its thumbnail under the hash
    /// of the normalized bytes. A re-copy finds both files there already.
    static func store(_ data: Data) -> StoredClipboardImage? {
        // Dimensions off the header first: the byte limit does not bound what
        // those bytes decode to, and everything below this decodes them.
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
            let pixelSize = pixelSize(in: source),
            pixelSize.width * pixelSize.height <= CGFloat(Constants.maxPixelCount),
            let png = pngData(from: data, source: source)
        else {
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

    /// Core sweeps by hash prefix, so every name has to start with one.
    private static func url(forHash hash: String, thumbnail: Bool) -> URL? {
        guard !hash.isEmpty, let directory else { return nil }
        let suffix = thumbnail ? Constants.thumbnailSuffix : ""
        return directory.appendingPathComponent("\(hash)\(suffix).\(Constants.fileExtension)")
    }

    /// One encoding for every clip, so a picture pasted as PNG and as TIFF is
    /// one row. Already-PNG bytes are kept verbatim.
    private static func pngData(from data: Data, source: CGImageSource) -> Data? {
        if let type = CGImageSourceGetType(source), (type as String) == UTType.png.identifier {
            return data
        }
        guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        return encodePNG(image)
    }

    /// From the header, without decoding the pixels.
    static func pixelSize(ofFileAt url: URL) -> CGSize? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
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

    /// A row draws this, not the full image, which would stutter the list.
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

nonisolated struct ClipboardImageCandidate {
    let data: Data
    /// Set only when the image arrived as a file. Raw pixels have no name.
    let fileName: String?
}

/// Decides only whether a copy was an image, so the polling loop stays one
/// method.
enum ClipboardImagePasteboard {
    private typealias Constants = AppConstants.Launcher.ClipboardImage

    /// Files first: an image copied in Finder carries both a file URL and a
    /// preview, and the filename is the better label.
    static func candidate(
        from pasteboard: NSPasteboard, types: [NSPasteboard.PasteboardType]
    ) -> ClipboardImageCandidate? {
        fileCandidate(from: pasteboard) ?? rawCandidate(from: pasteboard, types: types)
    }

    /// For pixels with no filename: the app and the time, so a list of them can
    /// still be told apart.
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
        // Size from the directory entry, before the read: this is the main
        // thread, and reading a huge file to reject it would freeze it.
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
