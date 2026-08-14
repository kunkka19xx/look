import AppKit
import Combine
import Foundation

/// Serializes clipboard writes: unordered detached tasks let a copy land after
/// the clear meant to erase it.
private actor ClipboardWriter {
    static let shared = ClipboardWriter()

    func record(content: String, appBundleID: String?) -> Int64? {
        EngineBridge.shared.recordClipboard(content: content, appBundleID: appBundleID)
    }

    func delete(id: Int64) {
        EngineBridge.shared.deleteClipboardEntry(id: id)
    }

    func clear() {
        EngineBridge.shared.clearClipboardHistory()
    }
}

struct ClipboardHistoryEntry: Identifiable, Equatable {
    let id: UUID
    let content: String
    let capturedAt: Date
    /// Derived once at capture time, not per render: rescanning full content on
    /// every body evaluation stutters keyboard navigation.
    let title: String
    let lineCount: Int
    let characterCount: Int
    /// What re-copying this entry actually pastes, when it differs from
    /// `content` (a labeled entry like `2+2 = 4` pastes `4`). Nil for entries
    /// captured passively off the system pasteboard.
    let payload: String?
    /// Row id in the persisted history, when this clip came from (or reached)
    /// the database. Without it, deleting a clip here would leave it in the
    /// stored corpus - a clip the user believes they erased.
    let storeID: Int64?

    init(
        id: UUID = UUID(),
        content: String,
        capturedAt: Date = Date(),
        payload: String? = nil,
        storeID: Int64? = nil
    ) {
        self.id = id
        self.content = content
        self.capturedAt = capturedAt
        self.storeID = storeID
        self.title = Self.makeTitle(from: content)
        self.lineCount = Self.makeLineCount(from: content)
        self.characterCount = content.count
        self.payload = payload
    }

    /// CRLF and bare CR both read as one line break (terminal output carries CR).
    private static func normalizedNewlines(_ content: String) -> String {
        content
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
    }

    /// Blank lines count, and a single trailing newline does not add one.
    private static func makeLineCount(from content: String) -> Int {
        var normalized = normalizedNewlines(content)
        if normalized.hasSuffix("\n") {
            normalized.removeLast()
        }
        guard !normalized.isEmpty else { return 1 }
        return normalized.split(separator: "\n", omittingEmptySubsequences: false).count
    }

    private static func makeTitle(from content: String) -> String {
        let collapsed = normalizedNewlines(content)
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
        loadPersistedHistory()
    }

    /// History outlives the app now: the in-memory list is seeded from the
    /// stored corpus so `c"` shows what was copied before the last quit.
    private func loadPersistedHistory() {
        let limit = maxEntries
        Task { [weak self] in
            let stored = await Task.detached(priority: .utility) {
                EngineBridge.shared.clipboardEntries(limit: limit)
            }.value
            guard let self, self.entries.isEmpty else { return }
            self.entries = stored.map {
                ClipboardHistoryEntry(
                    content: $0.content, capturedAt: $0.copiedAt, storeID: $0.id)
            }
        }
    }

    /// Off the main thread, but the row id comes BACK: without it `deleteEntry`
    /// has nothing to remove on disk and the clip returns next launch.
    private func persist(_ content: String, entryID: UUID) {
        let app = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        Task { [weak self] in
            let storeID = await ClipboardWriter.shared.record(content: content, appBundleID: app)
            guard let self, let storeID else { return }
            self.attachStoreID(storeID, to: entryID)
        }
    }

    /// Rebuilds the entry with its persisted id, or forgets the row when the
    /// entry was deleted while the write was in flight.
    private func attachStoreID(_ storeID: Int64, to entryID: UUID) {
        guard let index = entries.firstIndex(where: { $0.id == entryID }) else {
            // The user deleted it before the write landed: forget it on disk
            // too, or it comes back.
            Task { await ClipboardWriter.shared.delete(id: storeID) }
            return
        }
        let existing = entries[index]
        entries[index] = ClipboardHistoryEntry(
            id: existing.id,
            content: existing.content,
            capturedAt: existing.capturedAt,
            payload: existing.payload,
            storeID: storeID)
    }

    /// Forgets every clip, in memory and on disk. The promise that makes
    /// persisting clipboard history acceptable in the first place.
    func clearHistory() {
        entries.removeAll()
        Task { await ClipboardWriter.shared.clear() }
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
        let storeIDs = entries.filter { $0.id == id }.compactMap(\.storeID)
        entries.removeAll { $0.id == id }
        guard !storeIDs.isEmpty else { return }
        Task {
            for storeID in storeIDs { await ClipboardWriter.shared.delete(id: storeID) }
        }
    }

    /// Copies `payload` to the pasteboard but files it in history under
    /// `display` (e.g. the calculator row's `2+2 = 4`); re-copying the entry
    /// still pastes `payload`. Marks the pasteboard change as already seen so
    /// the passive poller doesn't also insert an unlabeled duplicate.
    func recordLabeled(display: String, payload: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(payload, forType: .string)
        lastChangeCount = pasteboard.changeCount

        entries.removeAll { $0.content == display }
        let labeled = ClipboardHistoryEntry(content: display, payload: payload)
        prepend(labeled)
        persist(display, entryID: labeled.id)
    }

    /// Inserts `entry` at the front and drops anything beyond `maxEntries`.
    private func prepend(_ entry: ClipboardHistoryEntry) {
        entries.insert(entry, at: 0)
        if entries.count > maxEntries {
            entries.removeLast(entries.count - maxEntries)
        }
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

        // One cross-process read of the type list, shared by both gates: this
        // runs on every copy made anywhere in the OS.
        let types = pasteboard.types ?? []
        if typesCarryFileReference(types, pasteboard) { return }
        // Checked BEFORE the text is read, let alone stored: a password
        // manager marks its clip concealed precisely so history tools skip it,
        // and history is now written to disk. The core cannot enforce this -
        // pasteboard markers only exist on this side.
        if typesAreConcealed(types) { return }

        guard var text = pasteboard.string(forType: .string) else { return }
        if text.count > maxStoredCharacters {
            let originalCount = text.count
            text = String(text.prefix(maxStoredCharacters))
            text += "\n\n[truncated from \(originalCount) chars]"
        }

        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }

        let captured: ClipboardHistoryEntry
        if let existingIndex = entries.firstIndex(where: { $0.content == text }) {
            // Keep the existing id so SwiftUI identity survives the re-capture.
            let existing = entries.remove(at: existingIndex)
            captured = ClipboardHistoryEntry(
                id: existing.id, content: text, storeID: existing.storeID)
        } else {
            captured = ClipboardHistoryEntry(content: text)
        }
        prepend(captured)
        // Re-copying replaces the stored row, so the id is re-attached here
        // rather than assumed to be the one this entry already carried.
        persist(text, entryID: captured.id)
    }

    /// The `org.nspasteboard.*` convention: apps mark clips that history tools
    /// must not keep. Concealed is a secret (password managers), transient is
    /// a fleeting intermediate, auto-generated was not typed by the user.
    private static let concealedMarkers: Set<String> = [
        "org.nspasteboard.ConcealedType",
        "org.nspasteboard.TransientType",
        "org.nspasteboard.AutoGeneratedType",
    ]

    private func typesAreConcealed(_ types: [NSPasteboard.PasteboardType]) -> Bool {
        types.contains { Self.concealedMarkers.contains($0.rawValue) }
    }

    private func typesCarryFileReference(
        _ types: [NSPasteboard.PasteboardType], _ pasteboard: NSPasteboard
    ) -> Bool {
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
