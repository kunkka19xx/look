import AppKit
import Darwin
import UserNotifications

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let hotKeyManager = GlobalHotKeyManager()
    private let pomoMenuBarItem = PomoMenuBarItem()

    // Grace period allows macOS "Quit & Reopen" handoff to release the previous process lock.
    private static let relaunchGracePeriodSeconds: TimeInterval = 0.8
    private static let contentionRetrySeconds: TimeInterval = 0.25
    private static let lockPollIntervalMicros: useconds_t = 50_000
    nonisolated(unsafe) private static var singletonLockFD: CInt = -1

    deinit {
        SingleInstanceLock.release(Self.singletonLockFD)
        Self.singletonLockFD = -1
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        if shouldTerminateDuplicateInstance() {
            NSApp.terminate(nil)
            return
        }

        hotKeyManager.registerToggleHotKey()
        NSApp.setActivationPolicy(.accessory)
        pomoMenuBarItem.install()

        // Notifications: ask for permission early (so the prompt isn't
        // tied to the user being mid-pomodoro) and forward foreground
        // deliveries through a delegate so banners aren't suppressed
        // when the launcher window is the active app.
        UNUserNotificationCenter.current().delegate = PomoNotifications.foregroundDelegate
        PomoNotifications.requestPermissionEarly()
    }

    private func shouldTerminateDuplicateInstance() -> Bool {
        let currentBundlePath = Bundle.main.bundleURL.resolvingSymlinksInPath().path
        let lockPath = SingleInstanceLock.lockPath(for: currentBundlePath)

        // Try to acquire singleton lock with grace period for "Quit & Reopen" handoff
        let lockResult = acquireSingletonLock(lockPath: lockPath, timeoutSeconds: Self.relaunchGracePeriodSeconds)

        if case .heldByOtherInstance = lockResult {
            if checkAndActivateDuplicateInstance(currentBundlePath: currentBundlePath) {
                return true
            }

            let retryResult = acquireSingletonLock(lockPath: lockPath, timeoutSeconds: Self.contentionRetrySeconds)
            if case .heldByOtherInstance = retryResult {
                return checkAndActivateDuplicateInstance(currentBundlePath: currentBundlePath)
            }
        }

        // Always check for other running instances to handle:
        // 1. Mixed-version scenarios (older builds not using lock protocol)
        // 2. Lock subsystem unavailable (fallback to process-based detection)
        return checkAndActivateDuplicateInstance(currentBundlePath: currentBundlePath)
    }

    private func checkAndActivateDuplicateInstance(currentBundlePath: String) -> Bool {
        guard let bundleIdentifier = Bundle.main.bundleIdentifier else {
            return false
        }

        let currentPID = ProcessInfo.processInfo.processIdentifier
        let runningApps = NSRunningApplication.runningApplications(withBundleIdentifier: bundleIdentifier)
        let otherInstances = runningApps.filter { $0.processIdentifier != currentPID }

        // No other instances found
        guard !otherInstances.isEmpty else {
            return false
        }

        // Prefer instance at same path (clean handoff for "Quit & Reopen")
        // Fall back to any instance if same path not found (prevents concurrent instances from different paths)
        let samePathInstance = otherInstances.first { app in
            let otherPath = app.bundleURL?.resolvingSymlinksInPath().path
            return otherPath == currentBundlePath
        }

        let primaryApp = samePathInstance ?? otherInstances.min(by: { $0.processIdentifier < $1.processIdentifier })!

        primaryApp.activate(options: [.activateAllWindows])
        return true
    }

    private func acquireSingletonLock(lockPath: String, timeoutSeconds: TimeInterval) -> SingleInstanceLockResult {
        if Self.singletonLockFD >= 0 {
            return .acquired(Self.singletonLockFD)
        }

        let lockResult = SingleInstanceLock.acquire(
            lockPath: lockPath,
            timeoutSeconds: timeoutSeconds,
            pollIntervalMicros: Self.lockPollIntervalMicros
        )

        if case .acquired(let fd) = lockResult {
            Self.singletonLockFD = fd
        }

        return lockResult
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if let window = collapseToSingleLauncherWindow(preferred: sender.windows.first(where: { $0.isVisible })) {
            sender.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
        }
        NotificationCenter.default.post(name: .lookActivateLauncherRequested, object: nil)
        // We handled reopen ourselves; prevent AppKit from creating another window.
        return false
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        DispatchQueue.main.async {
            if let app = notification.object as? NSApplication,
                let window = app.windows.first(where: { $0.isVisible }) ?? app.windows.first
            {
                app.activate(ignoringOtherApps: true)
                window.makeKeyAndOrderFront(nil)
            }
            NotificationCenter.default.post(name: .lookActivateLauncherRequested, object: nil)
        }
    }

    @discardableResult
    private func collapseToSingleLauncherWindow(preferred: NSWindow? = nil) -> NSWindow? {
        let windows = NSApplication.shared.windows
        guard !windows.isEmpty else { return nil }

        let primary = preferred ?? windows.first(where: { $0.isVisible }) ?? windows.first
        guard let primary else { return nil }

        for window in windows where window !== primary {
            window.orderOut(nil)
            window.close()
        }

        return primary
    }
}
