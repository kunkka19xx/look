import AppKit
import Combine
import Foundation
import OSLog

enum RunningAppsLog {
    static let logger = Logger(subsystem: "noah-code.Look", category: "running-apps")
}

struct RunningAppItem: Identifiable, Equatable {
    let id: pid_t
    let bundleIdentifier: String?
    let name: String
    let icon: NSImage?

    static func == (lhs: RunningAppItem, rhs: RunningAppItem) -> Bool {
        lhs.id == rhs.id && lhs.name == rhs.name && lhs.bundleIdentifier == rhs.bundleIdentifier
    }
}

@MainActor
final class RunningAppsService: ObservableObject {
    @Published private(set) var items: [RunningAppItem] = []
    @Published private(set) var activePID: pid_t?

    private let ownPID = ProcessInfo.processInfo.processIdentifier

    init() {
        attachNotifications()
        refresh()
    }

    func refresh() {
        let frontmost = NSWorkspace.shared.frontmostApplication?.processIdentifier
        let running = NSWorkspace.shared.runningApplications
            .filter { $0.activationPolicy == .regular }
            .filter { $0.processIdentifier != ownPID }

        let snapshot: [RunningAppItem] = running.map { app in
            RunningAppItem(
                id: app.processIdentifier,
                bundleIdentifier: app.bundleIdentifier,
                name: app.localizedName ?? app.bundleIdentifier ?? "App",
                icon: app.icon
            )
        }

        // Alphabetical, stable. Reordering on activation was forcing the
        // user to re-scan the strip after every switch - keep positions
        // fixed so the Cmd+N key for "Discord" stays the same.
        let sorted = snapshot.sorted { lhs, rhs in
            lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }
        items = Array(sorted.prefix(AppConstants.Launcher.RunningAppsStrip.maxItems))

        if let frontmost, frontmost != ownPID {
            activePID = frontmost
        }
    }

    @discardableResult
    func activate(index: Int) -> Bool {
        let log = RunningAppsLog.logger
        guard index >= 0, index < items.count else {
            log.debug("activate(index: \(index, privacy: .public)) - out of range (count=\(self.items.count, privacy: .public))")
            return false
        }
        let item = items[index]
        guard let app = NSRunningApplication(processIdentifier: item.id) else {
            log.debug("activate \(item.name, privacy: .public) pid=\(item.id, privacy: .public) - NSRunningApplication lookup returned nil (process gone?)")
            return false
        }
        guard !app.isTerminated else {
            log.debug("activate \(item.name, privacy: .public) pid=\(item.id, privacy: .public) - already terminated")
            return false
        }
        let wasHidden = app.isHidden
        if wasHidden {
            app.unhide()
        }
        // Detect Finder-style apps that show up in the running list but
        // have no visible window. Activating them alone would flash the
        // launcher closed with nothing visibly happening. Track this so
        // we can follow up with a `NSWorkspace.openApplication(...)`
        // which fires the same "reopen" event a Dock click sends -
        // those apps respond by spawning a fresh window.
        let hadVisibleWindow = Self.hasOnScreenWindow(pid: app.processIdentifier)
        let result = app.activate(options: [.activateAllWindows])
        log.debug("activate \(item.name, privacy: .public) pid=\(item.id, privacy: .public) wasHidden=\(wasHidden, privacy: .public) hadVisibleWindow=\(hadVisibleWindow, privacy: .public) - activate returned \(result, privacy: .public)")
        if result, !hadVisibleWindow, let bundleURL = app.bundleURL {
            let configuration = NSWorkspace.OpenConfiguration()
            configuration.activates = true
            configuration.addsToRecentItems = false
            NSWorkspace.shared.openApplication(at: bundleURL, configuration: configuration) { _, error in
                if let error {
                    log.debug("openApplication(at:) follow-up failed for \(item.name, privacy: .public): \(error.localizedDescription, privacy: .public)")
                }
            }
        }
        return result
    }

    /// Returns true if the given pid owns at least one on-screen,
    /// non-desktop-element window. Used to detect apps that are in the
    /// "running but no window" state (Finder is the canonical example)
    /// so we can follow `activate()` with a reopen request.
    private static func hasOnScreenWindow(pid: pid_t) -> Bool {
        let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
        guard let infos = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
            return false
        }
        let key = kCGWindowOwnerPID as String
        return infos.contains { info in
            guard let ownerPID = info[key] as? pid_t else { return false }
            return ownerPID == pid
        }
    }

    private func attachNotifications() {
        let nc = NSWorkspace.shared.notificationCenter

        nc.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] note in
            let activatedPID = (note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication)?.processIdentifier
            Task { @MainActor in
                guard let self else { return }
                // Track which app is frontmost (for the accent ring) but
                // do NOT reorder items - positions are kept stable.
                if let pid = activatedPID { self.activePID = pid }
                self.refresh()
            }
        }

        for name in [
            NSWorkspace.didLaunchApplicationNotification,
            NSWorkspace.didTerminateApplicationNotification,
        ] {
            nc.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in self?.refresh() }
            }
        }
    }
}
