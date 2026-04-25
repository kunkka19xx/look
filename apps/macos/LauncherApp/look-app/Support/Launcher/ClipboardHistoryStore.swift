import AppKit
import Combine
import Foundation

struct ClipboardHistoryEntry: Identifiable, Equatable {
    let id: UUID
    let content: String
    let capturedAt: Date

    init(id: UUID = UUID(), content: String, capturedAt: Date = Date()) {
        self.id = id
        self.content = content
        self.capturedAt = capturedAt
    }

    var title: String {
        let collapsed = content
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if collapsed.isEmpty {
            return "(Empty text)"
        }
        if collapsed.count <= 80 {
            return collapsed
        }
        return String(collapsed.prefix(80)) + "…"
    }

    var lineCount: Int {
        max(1, content.split(whereSeparator: \.isNewline).count)
    }

    var characterCount: Int {
        content.count
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

    private let maxEntries = AppConstants.Launcher.Clipboard.maxEntries
    private let maxStoredCharacters = AppConstants.Launcher.Clipboard.maxStoredCharacters
    private var monitoringMode: MonitoringMode = .foreground
    private var timer: Timer?
    private var burstTimer: Timer?
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
            self?.captureLatestClipboardIfNeeded()
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
        ) { [weak self] timer in
            guard let self else {
                timer.invalidate()
                return
            }

            if self.remainingBurstSamples <= 0 {
                timer.invalidate()
                self.burstTimer = nil
                return
            }

            self.remainingBurstSamples -= 1
            self.captureLatestClipboardIfNeeded()
        }

        if let burstTimer {
            RunLoop.main.add(burstTimer, forMode: .common)
        }
    }
}
