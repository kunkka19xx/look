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

    /// One hop off the main thread: hashing and encoding a screenshot would
    /// stutter the poll loop that found it.
    func recordImage(data: Data, label: String, appBundleID: String?)
        -> (stored: StoredClipboardImage, storeID: Int64)?
    {
        guard let stored = ClipboardImageFiles.store(data),
            let storeID = EngineBridge.shared.recordClipboardImage(
                label: label, imageHash: stored.hash, appBundleID: appBundleID)
        else {
            return nil
        }
        return (stored, storeID)
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
    /// Apart from `entries` because the two share no search key and no row
    /// shape. The storage underneath is one table.
    @Published private(set) var imageEntries: [ClipboardImageEntry] = []

    private var maxEntries = ClipboardHistoryStore.resolveMaxEntries()
    private var maxImageEntries = ClipboardHistoryStore.resolveMaxImageEntries()
    /// Bumped by `clearHistory`. The writer actor serializes its calls but does
    /// not order them, so a capture can land after the clear meant to erase it.
    /// Text needs no counter: `attachStoreID` already drops a row whose entry
    /// has left the list.
    private var historyGeneration = 0
    private let maxStoredCharacters = AppConstants.Launcher.Clipboard.maxStoredCharacters

    /// Re-reads the clipboard section of `~/.look/config` and applies it live, so file-only
    /// clipboard settings take effect on config reload (`Cmd+Shift+;`) without a restart.
    /// Matches the `reloadFromConfig()` convention used by ThemeStore. Every clipboard key
    /// is applied from a single parse here, so adding a key is one more `apply` line below,
    /// not a new reload method.
    func reloadFromConfig() {
        let values = ClipboardHistoryStore.loadConfigValues()
        applyMaxEntries(ClipboardHistoryStore.resolveMaxEntries(from: values))
        applyMaxImageEntries(ClipboardHistoryStore.resolveMaxImageEntries(from: values))
    }

    private func applyMaxEntries(_ newValue: Int) {
        guard newValue != maxEntries else { return }
        maxEntries = newValue
        Self.trim(&entries, to: maxEntries)
    }

    private func applyMaxImageEntries(_ newValue: Int) {
        guard newValue != maxImageEntries else { return }
        maxImageEntries = newValue
        Self.trim(&imageEntries, to: maxImageEntries)
    }

    private static func trim<Row>(_ rows: inout [Row], to limit: Int) {
        guard rows.count > limit else { return }
        rows.removeLast(rows.count - limit)
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
        typealias Clipboard = AppConstants.Launcher.Clipboard
        return resolveLimit(
            from: values,
            key: Clipboard.historyLimitConfigKey,
            range: Clipboard.minEntries...Clipboard.maxEntriesLimit,
            fallback: Clipboard.maxEntries)
    }

    private static func resolveMaxImageEntries() -> Int {
        resolveMaxImageEntries(from: loadConfigValues())
    }

    /// Capped separately from text, and far lower: megabytes, not characters.
    private static func resolveMaxImageEntries(from values: [String: String]) -> Int {
        typealias Image = AppConstants.Launcher.ClipboardImage
        return resolveLimit(
            from: values,
            key: Image.limitConfigKey,
            range: Image.minEntries...Image.maxEntriesLimit,
            fallback: Image.maxEntries)
    }

    /// An out-of-range limit is a typo, not a request, so the default stands
    /// rather than the nearest bound: clamping 1000 to 100 looks like it worked.
    private static func resolveLimit(
        from values: [String: String], key: String, range: ClosedRange<Int>, fallback: Int
    ) -> Int {
        guard let rawValue = values[key],
            let parsed = Int(rawValue.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            return fallback
        }
        return range.contains(parsed) ? parsed : fallback
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
        let imageLimit = maxImageEntries
        Task { [weak self] in
            let stored = await Task.detached(priority: .utility) {
                (
                    text: EngineBridge.shared.clipboardEntries(limit: limit),
                    // Mapped here: each image row stats its file.
                    images: EngineBridge.shared.clipboardEntries(
                        kind: AppConstants.Launcher.Clipboard.imageKind, limit: imageLimit
                    ).compactMap(Self.restoredImageEntry)
                )
            }.value
            guard let self else { return }
            if self.entries.isEmpty {
                self.entries = stored.text.map {
                    ClipboardHistoryEntry(
                        content: $0.content, capturedAt: $0.copiedAt, storeID: $0.id)
                }
            }
            if self.imageEntries.isEmpty {
                self.imageEntries = stored.images
            }
        }
    }

    /// A row whose file is gone is dropped rather than shown broken: the bytes
    /// are the clip, and without them there is nothing to paste.
    nonisolated private static func restoredImageEntry(from stored: EngineBridge.ClipboardEntry)
        -> ClipboardImageEntry?
    {
        guard let url = ClipboardImageFiles.fileURL(forHash: stored.contentHash),
            let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
            let byteSize = attributes[.size] as? Int
        else {
            return nil
        }
        return ClipboardImageEntry(
            label: stored.content,
            hash: stored.contentHash,
            capturedAt: stored.copiedAt,
            pixelSize: ClipboardImageFiles.pixelSize(ofFileAt: url) ?? .zero,
            byteSize: byteSize,
            appBundleID: stored.appBundleID,
            storeID: stored.id)
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
        imageEntries.removeAll()
        historyGeneration += 1
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
        Self.trim(&entries, to: maxEntries)
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
        // Checked BEFORE the text is read, let alone stored: a password
        // manager marks its clip concealed precisely so history tools skip it,
        // and history is now written to disk. The core cannot enforce this -
        // pasteboard markers only exist on this side. Images land on disk too,
        // so it runs before either branch.
        if typesAreConcealed(types) { return }

        // Before the file bail: an image copied in Finder IS a file reference.
        if captureImageIfPresent(pasteboard, types: types) { return }
        if typesCarryFileReference(types, pasteboard) { return }

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

    /// Files and records a copied image, reporting whether the copy was one.
    private func captureImageIfPresent(
        _ pasteboard: NSPasteboard, types: [NSPasteboard.PasteboardType]
    ) -> Bool {
        guard let candidate = ClipboardImagePasteboard.candidate(from: pasteboard, types: types)
        else {
            return false
        }
        // The copy WAS an image, so it never falls through to the text branch,
        // whether or not it is one worth keeping.
        guard candidate.data.count <= AppConstants.Launcher.ClipboardImage.maxImageBytes else {
            return true
        }

        let source = NSWorkspace.shared.frontmostApplication
        let capturedAt = Date()
        let label =
            candidate.fileName
            ?? ClipboardImagePasteboard.label(
                forSourceNamed: source?.localizedName, capturedAt: capturedAt)
        let appBundleID = source?.bundleIdentifier
        let generation = historyGeneration
        Task { [weak self] in
            let recorded = await ClipboardWriter.shared.recordImage(
                data: candidate.data, label: label, appBundleID: appBundleID)
            guard let self, let recorded else { return }
            guard generation == self.historyGeneration else {
                // Cleared while in flight: forget it on disk too, or the clip
                // the user erased comes back with its file.
                await ClipboardWriter.shared.delete(id: recorded.storeID)
                return
            }
            self.prependImage(
                ClipboardImageEntry(
                    label: label,
                    hash: recorded.stored.hash,
                    capturedAt: capturedAt,
                    pixelSize: recorded.stored.pixelSize,
                    byteSize: recorded.stored.byteSize,
                    appBundleID: appBundleID,
                    storeID: recorded.storeID))
        }
        return true
    }

    /// Drops any older row for the same pixels: the store dedupes by hash, so
    /// two rows here would be one row on disk.
    private func prependImage(_ entry: ClipboardImageEntry) {
        imageEntries.removeAll { $0.hash == entry.hash }
        imageEntries.insert(entry, at: 0)
        Self.trim(&imageEntries, to: maxImageEntries)
    }

    func searchImages(_ term: String) -> [ClipboardImageEntry] {
        let normalized = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return imageEntries }
        return imageEntries.filter { $0.label.localizedCaseInsensitiveContains(normalized) }
    }

    func deleteImageEntry(id: String) {
        let storeIDs = imageEntries.filter { $0.id == id }.compactMap(\.storeID)
        imageEntries.removeAll { $0.id == id }
        guard !storeIDs.isEmpty else { return }
        // Core unlinks the file with the row: the pixels are what was deleted.
        Task {
            for storeID in storeIDs { await ClipboardWriter.shared.delete(id: storeID) }
        }
    }

    /// Both pixels and a file, so it pastes into an editor and into Finder
    /// alike. Marks the change seen so the poller skips Look's own write.
    @discardableResult
    func copyImage(entry: ClipboardImageEntry) -> Bool {
        guard let url = entry.fileURL, let image = NSImage(contentsOf: url) else { return false }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let wrote = pasteboard.writeObjects([image, url as NSURL])
        lastChangeCount = pasteboard.changeCount
        return wrote
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
