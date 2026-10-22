import Foundation

/// The `ci"` screen. A sibling of LauncherClipboardFeature, not a branch in it:
/// the two share storage but nothing else, since text rows are searched by
/// content and measured in lines, image rows by name and drawn as pictures.
enum LauncherClipboardImageFeature {
    static func isClipboardImageQuery(_ query: String) -> Bool {
        ClipboardQueryPrefix.image.matches(query)
    }

    static func searchTerm(from query: String) -> String? {
        ClipboardQueryPrefix.image.searchTerm(in: query)
    }

    static func makeResult(entry: ClipboardImageEntry, dateFormatter: DateFormatter)
        -> LauncherResult
    {
        let timestamp = dateFormatter.string(from: entry.capturedAt)
        let dimensions = "\(Int(entry.pixelSize.width))×\(Int(entry.pixelSize.height))"
        let size = ByteCountFormatter.string(
            fromByteCount: Int64(entry.byteSize), countStyle: .file)
        let subtitle = "Image  •  \(dimensions)  •  \(size)  •  \(timestamp)"

        var result = LauncherResult(
            id: "\(AppConstants.Launcher.ClipboardImage.resultIDPrefix)\(entry.id)",
            kind: .clipboard,
            title: entry.label,
            subtitle: subtitle,
            path: AppConstants.Launcher.ClipboardImage.resultPath,
            score: 0
        )
        result.clipboardCapturedAt = entry.capturedAt
        result.clipboardImagePath = entry.fileURL?.path
        result.clipboardImageThumbnailPath = entry.thumbnailURL?.path
        result.clipboardImagePixelSize = entry.pixelSize
        result.clipboardImageByteSize = entry.byteSize
        return result
    }

    /// The image hash a row id carries, or nil when the id is another row's.
    static func entryID(fromResultID resultID: String) -> String? {
        let idPrefix = AppConstants.Launcher.ClipboardImage.resultIDPrefix
        guard resultID.hasPrefix(idPrefix) else { return nil }
        let hash = String(resultID.dropFirst(idPrefix.count))
        return hash.isEmpty ? nil : hash
    }
}
