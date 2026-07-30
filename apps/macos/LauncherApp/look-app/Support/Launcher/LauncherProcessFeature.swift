import AppKit
import UniformTypeIdentifiers

/// Query detection, row construction, and icon resolution for the `ps"` process
/// finder. Mirrors `LauncherClipboardFeature`: `AppConstants.Launcher.Process`
/// holds the constants, this owns the logic.
enum LauncherProcessFeature {
    static func isProcessQuery(_ query: String) -> Bool {
        query.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .hasPrefix(AppConstants.Launcher.QueryPrefix.process)
    }

    /// The text typed after `ps"` (empty when just the prefix is present), or
    /// nil when `query` isn't a process query.
    static func searchTerm(from query: String) -> String? {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let prefix = AppConstants.Launcher.QueryPrefix.process
        guard trimmed.lowercased().hasPrefix(prefix) else { return nil }
        return String(trimmed.dropFirst(prefix.count)).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// A results-list row for a process. Subtitle is `PID <pid>` plus `· :port`
    /// per listening port (matches the linows row).
    static func makeResult(_ candidate: ProcessScoring.Candidate) -> LauncherResult {
        var subtitle = "PID \(candidate.pid)"
        if !candidate.ports.isEmpty {
            subtitle += " · " + candidate.ports.map { ":\($0)" }.joined(separator: " ")
        }
        var result = LauncherResult(
            id: "\(AppConstants.Launcher.Process.resultIDPrefix)\(candidate.pid)",
            kind: .process,
            title: candidate.name.isEmpty ? "Process \(candidate.pid)" : candidate.name,
            subtitle: subtitle,
            path: AppConstants.Launcher.Process.resultPath,
            score: 0
        )
        result.processPID = candidate.pid
        result.processPorts = candidate.ports
        return result
    }

    /// The pid encoded in a process-row id, or nil.
    static func pid(fromResultID resultID: String) -> Int32? {
        let prefix = AppConstants.Launcher.Process.resultIDPrefix
        guard resultID.hasPrefix(prefix) else { return nil }
        return Int32(resultID.dropFirst(prefix.count))
    }

    /// App-backed process → its app icon; otherwise a generic process glyph.
    /// Single source for the row, preview, and `/kill` list.
    static func icon(forPID pid: Int32) -> NSImage {
        if let app = NSRunningApplication(processIdentifier: pid), let icon = app.icon {
            return icon
        }
        return NSImage(systemSymbolName: "gearshape", accessibilityDescription: nil)
            ?? NSWorkspace.shared.icon(for: .unixExecutable)
    }
}
