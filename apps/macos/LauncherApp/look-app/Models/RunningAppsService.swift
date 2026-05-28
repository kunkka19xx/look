import AppKit
import Combine
import Foundation

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
    // Most-recently-active first. Bundle ID preferred; falls back to pid for apps without one.
    private var recencyOrder: [String] = []

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

        let sorted = sortByRecency(snapshot)
        items = Array(sorted.prefix(AppConstants.Launcher.RunningAppsStrip.maxItems))

        if let frontmost, frontmost != ownPID {
            activePID = frontmost
            promote(pid: frontmost)
        }
    }

    func activate(index: Int) {
        guard index >= 0, index < items.count else { return }
        let item = items[index]
        guard let app = NSRunningApplication(processIdentifier: item.id) else { return }
        guard !app.isTerminated else { return }
        _ = app.activate()
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
                if let pid = activatedPID {
                    self.promote(pid: pid)
                    self.activePID = pid
                }
                self.refresh()
            }
        }

        nc.addObserver(
            forName: NSWorkspace.didLaunchApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        }

        nc.addObserver(
            forName: NSWorkspace.didTerminateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.refresh() }
        }
    }

    private func recencyKey(for item: RunningAppItem) -> String {
        item.bundleIdentifier ?? "pid:\(item.id)"
    }

    private func promote(pid: pid_t) {
        guard let app = NSRunningApplication(processIdentifier: pid) else { return }
        let key = app.bundleIdentifier ?? "pid:\(pid)"
        recencyOrder.removeAll { $0 == key }
        recencyOrder.insert(key, at: 0)
    }

    private func sortByRecency(_ items: [RunningAppItem]) -> [RunningAppItem] {
        let order = recencyOrder
        return items.sorted { lhs, rhs in
            let lk = recencyKey(for: lhs)
            let rk = recencyKey(for: rhs)
            let li = order.firstIndex(of: lk) ?? Int.max
            let ri = order.firstIndex(of: rk) ?? Int.max
            if li != ri { return li < ri }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }
    }
}
