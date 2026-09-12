import Foundation

enum LauncherClipboardFeature {
    static func isClipboardQuery(_ query: String) -> Bool {
        ClipboardQueryPrefix.text.matches(query)
    }

    static func searchTerm(from query: String) -> String? {
        ClipboardQueryPrefix.text.searchTerm(in: query)
    }

    static func makeResult(entry: ClipboardHistoryEntry, dateFormatter: DateFormatter) -> LauncherResult {
        let timestamp = dateFormatter.string(from: entry.capturedAt)
        let subtitle = "Clipboard  •  \(entry.characterCount) chars  •  \(entry.lineCount) lines  •  \(timestamp)"

        var result = LauncherResult(
            id: "\(AppConstants.Launcher.Clipboard.resultIDPrefix)\(entry.id.uuidString)",
            kind: .clipboard,
            title: entry.title,
            subtitle: subtitle,
            path: AppConstants.Launcher.Clipboard.resultPath,
            score: 0
        )
        result.clipboardContent = entry.content
        result.clipboardCapturedAt = entry.capturedAt
        result.clipboardCharacterCount = entry.characterCount
        result.clipboardLineCount = entry.lineCount
        result.clipboardPayload = entry.payload
        return result
    }

    static func entryID(fromResultID resultID: String) -> UUID? {
        let idPrefix = AppConstants.Launcher.Clipboard.resultIDPrefix
        guard resultID.hasPrefix(idPrefix) else { return nil }
        let rawID = String(resultID.dropFirst(idPrefix.count))
        return UUID(uuidString: rawID)
    }
}
