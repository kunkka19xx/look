import AppKit
import Foundation

final class ClipboardMonitor {
    static let shared = ClipboardMonitor()

    private var timer: Timer?
    private var lastChangeCount: Int
    private let bridge = EngineBridge.shared
    private let pollInterval: TimeInterval = 0.5

    private init() {
        lastChangeCount = NSPasteboard.general.changeCount
    }

    func start() {
        guard timer == nil else { return }
        lastChangeCount = NSPasteboard.general.changeCount

        timer = Timer.scheduledTimer(withTimeInterval: pollInterval, repeats: true) { [weak self] _ in
            self?.checkForChanges()
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    private func checkForChanges() {
        let current = NSPasteboard.general.changeCount
        guard current != lastChangeCount else { return }
        lastChangeCount = current

        let pasteboard = NSPasteboard.general
        let sourceApp = NSWorkspace.shared.frontmostApplication?.localizedName ?? ""

        if let fileURLs = pasteboard.readObjects(forClasses: [NSURL.self], options: [
            .urlReadingFileURLsOnly: true
        ]) as? [URL], !fileURLs.isEmpty {
            let paths = fileURLs.map(\.path).joined(separator: "\n")
            Task.detached(priority: .utility) { [bridge, sourceApp] in
                _ = bridge.storeClipboard(content: paths, contentType: "file_list", sourceApp: sourceApp)
            }
            return
        }

        if let string = pasteboard.string(forType: .string), !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            if isFromPasswordManager(string: string, pasteboard: pasteboard) {
                return
            }

            Task.detached(priority: .utility) { [bridge, sourceApp] in
                _ = bridge.storeClipboard(content: string, contentType: "text", sourceApp: sourceApp)
            }
            return
        }

        if pasteboard.data(forType: .tiff) != nil || pasteboard.data(forType: .png) != nil {
            if let imgData = pasteboard.data(forType: .png) ?? pasteboard.data(forType: .tiff) {
                let base64 = imgData.base64EncodedString()
                Task.detached(priority: .utility) { [bridge, sourceApp] in
                    _ = bridge.storeClipboard(content: base64, contentType: "image", sourceApp: sourceApp)
                }
            }
        }
    }

    private func isFromPasswordManager(string: String, pasteboard: NSPasteboard) -> Bool {
        if pasteboard.types?.contains(NSPasteboard.PasteboardType(rawValue: "org.nspasteboard.ConcealedType")) == true {
            return true
        }
        if pasteboard.types?.contains(NSPasteboard.PasteboardType(rawValue: "com.agilebits.onepassword")) == true {
            return true
        }
        return false
    }
}
