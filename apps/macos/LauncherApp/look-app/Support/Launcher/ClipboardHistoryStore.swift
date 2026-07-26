import AppKit
import Combine
import Foundation

struct ClipboardHistoryEntry: Identifiable, Equatable {
    let id: UUID
    let content: String
    let capturedAt: Date
    /// Derived once at capture time, not per render: rescanning full content on
    /// every body evaluation stutters keyboard navigation.
    let title: String
    let lineCount: Int
    let characterCount: Int

    init(id: UUID = UUID(), content: String, capturedAt: Date = Date()) {
        self.id = id
        self.content = content
        self.capturedAt = capturedAt
        self.title = Self.makeTitle(from: content)
        self.lineCount = Self.makeLineCount(from: content)
        self.characterCount = content.count
    }

    /// Blank lines count, and a single trailing newline does not add one.
    private static func makeLineCount(from content: String) -> Int {
        var normalized = content.replacingOccurrences(of: "\r\n", with: "\n")
        if normalized.hasSuffix("\n") {
            normalized.removeLast()
        }
        guard !normalized.isEmpty else { return 1 }
        return normalized.split(separator: "\n", omittingEmptySubsequences: false).count
    }

    private static func makeTitle(from content: String) -> String {
        let collapsed = content
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if collapsed.isEmpty {
            return AppConstants.Launcher.Clipboard.emptyEntryTitle
        }
        let limit = AppConstants.Launcher.Clipboard.maxTitleCharacters
        if collapsed.count <= limit {
            return collapsed
        }
        return String(collapsed.prefix(limit)) + "…"
    }
}

final class ClipboardHistoryStore: ObservableObject {
    enum MonitoringMode {
        case foreground
        case background

        var interval: TimeInterval {
            switch self {
            case .foreground:
                return AppConstants.Launcher.Clipboard.foregroundPollInterval
            case .background:
                return AppConstants.Launcher.Clipboard.backgroundPollInterval
            }
        }
    }

    @Published private(set) var entries: [ClipboardHistoryEntry] = []

    private var maxEntries = ClipboardHistoryStore.resolveMaxEntries()
    private let maxStoredCharacters = AppConstants.Launcher.Clipboard.maxStoredCharacters

    /// Re-reads the clipboard section of `~/.look.config` and applies it live, so file-only
    /// clipboard settings take effect on config reload (`Cmd+Shift+;`) without a restart.
    /// Matches the `reloadFromConfig()` convention used by ThemeStore. Every clipboard key
    /// is applied from a single parse here, so adding a key is one more `apply` line below,
    /// not a new reload method.
    func reloadFromConfig() {
        let values = ClipboardHistoryStore.loadConfigValues()
        applyMaxEntries(ClipboardHistoryStore.resolveMaxEntries(from: values))
    }

    private func applyMaxEntries(_ newValue: Int) {
        guard newValue != maxEntries else { return }
        maxEntries = newValue
        if entries.count > maxEntries {
            entries.removeLast(entries.count - maxEntries)
        }
    }

    /// Reads every `key=value` pair from the active config file once, or an empty map when
    /// the file is missing/unreadable. One parse feeds all clipboard settings.
    private static func loadConfigValues() -> [String: String] {
        let path = ConfigPathResolver.resolvedPath()
        guard let raw = try? String(contentsOfFile: path, encoding: .utf8) else {
            return [:]
        }
        return ConfigFileLines.keyValues(raw)
    }

    private static func resolveMaxEntries() -> Int {
        resolveMaxEntries(from: loadConfigValues())
    }

    /// Resolves `clipboard_history_limit` from already-parsed config values, falling back to
    /// the default (10) when the key is missing, unparseable, or outside the accepted
    /// [10, 100] range.
    private static func resolveMaxEntries(from values: [String: String]) -> Int {
        let fallback = AppConstants.Launcher.Clipboard.maxEntries
        guard let rawValue = values[AppConstants.Launcher.Clipboard.historyLimitConfigKey],
              let parsed = Int(rawValue.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            return fallback
        }
        let lower = AppConstants.Launcher.Clipboard.minEntries
        let upper = AppConstants.Launcher.Clipboard.maxEntriesLimit
        return (lower...upper).contains(parsed) ? parsed : fallback
    }
    private var monitoringMode: MonitoringMode = .foreground
    // nonisolated(unsafe) so the nonisolated deinit can call invalidate()
    // on these without going through the actor.
    nonisolated(unsafe) private var timer: Timer?
    nonisolated(unsafe) private var burstTimer: Timer?
    private var remainingBurstSamples = 0
    private var lastChangeCount: Int

    init() {
        lastChangeCount = NSPasteboard.general.changeCount
        startMonitoring()
    }

    deinit {
        timer?.invalidate()
        burstTimer?.invalidate()
    }

    func search(_ term: String) -> [ClipboardHistoryEntry] {
        let normalized = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return entries }

        return entries.filter { entry in
            entry.content.localizedCaseInsensitiveContains(normalized)
        }
    }

    func deleteEntry(id: UUID) {
        entries.removeAll { $0.id == id }
    }

    func setMonitoringMode(_ mode: MonitoringMode) {
        guard monitoringMode != mode else { return }
        monitoringMode = mode
        startMonitoring()
    }

    private func startMonitoring() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: monitoringMode.interval, repeats: true) { [weak self] _ in
            // Timer fires on RunLoop.main; assumeIsolated avoids a needless
            // Task hop while satisfying Swift 6's Sendable-closure check.
            MainActor.assumeIsolated {
                self?.captureLatestClipboardIfNeeded()
            }
        }
        if let timer {
            RunLoop.main.add(timer, forMode: .common)
        }
    }

    private func captureLatestClipboardIfNeeded() {
        let pasteboard = NSPasteboard.general
        guard pasteboard.changeCount != lastChangeCount else { return }
        lastChangeCount = pasteboard.changeCount

        startBurstCaptureWindow()

        if pasteboardCarriesFileReference(pasteboard) { return }

        guard var text = pasteboard.string(forType: .string) else { return }
        if text.count > maxStoredCharacters {
            let originalCount = text.count
            text = String(text.prefix(maxStoredCharacters))
            text += "\n\n[truncated from \(originalCount) chars]"
        }

        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }

        if let existingIndex = entries.firstIndex(where: { $0.content == text }) {
            let existing = entries.remove(at: existingIndex)
            let movedEntry = ClipboardHistoryEntry(id: existing.id, content: text)
            entries.insert(movedEntry, at: 0)
        } else {
            let newEntry = ClipboardHistoryEntry(content: text)
            entries.insert(newEntry, at: 0)
        }

        if entries.count > maxEntries {
            entries.removeLast(entries.count - maxEntries)
        }
    }

    private func pasteboardCarriesFileReference(_ pasteboard: NSPasteboard) -> Bool {
        let types = pasteboard.types ?? []
        if types.contains(.fileURL) { return true }
        if types.contains(NSPasteboard.PasteboardType("NSFilenamesPboardType")) { return true }
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL],
           !urls.isEmpty {
            return true
        }
        return false
    }

    private func startBurstCaptureWindow() {
        remainingBurstSamples = AppConstants.Launcher.Clipboard.burstSampleCount
        if burstTimer != nil {
            return
        }

        burstTimer = Timer.scheduledTimer(
            withTimeInterval: AppConstants.Launcher.Clipboard.burstPollInterval,
            repeats: true
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }

                if self.remainingBurstSamples <= 0 {
                    self.burstTimer?.invalidate()
                    self.burstTimer = nil
                    return
                }

                self.remainingBurstSamples -= 1
                self.captureLatestClipboardIfNeeded()
            }
        }

        if let burstTimer {
            RunLoop.main.add(burstTimer, forMode: .common)
        }
    }
}
