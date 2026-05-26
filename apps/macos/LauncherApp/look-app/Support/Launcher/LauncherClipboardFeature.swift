import Foundation

enum LauncherClipboardFeature {
    static func isClipboardQuery(_ query: String) -> Bool {
        query
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .hasPrefix(AppConstants.Launcher.QueryPrefix.clipboard)
    }

    static func searchTerm(from query: String) -> String? {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let prefix = AppConstants.Launcher.QueryPrefix.clipboard
        guard normalized.lowercased().hasPrefix(prefix) else { return nil }
        return String(normalized.dropFirst(prefix.count)).trimmingCharacters(in: .whitespacesAndNewlines)
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
        return result
    }

    static func entryID(fromResultID resultID: String) -> UUID? {
        let idPrefix = AppConstants.Launcher.Clipboard.resultIDPrefix
        guard resultID.hasPrefix(idPrefix) else { return nil }
        let rawID = String(resultID.dropFirst(idPrefix.count))
        return UUID(uuidString: rawID)
    }
}
