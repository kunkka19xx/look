import AppKit
import Darwin

final class AppDelegate: NSObject, NSApplicationDelegate {
    // Grace period allows macOS "Quit & Reopen" handoff to release the previous process lock.
    private static let relaunchGracePeriodSeconds: TimeInterval = 0.8
    private static let lockPollIntervalMicros: useconds_t = 50_000
    private static var singletonLockFD: CInt = -1

    private static func singletonLockPath(for bundlePath: String) -> String {
        let hash = stablePathHash(bundlePath)
        let fileName = "look-single-instance-\(hash).lock"
        return (NSTemporaryDirectory() as NSString).appendingPathComponent(fileName)
    }

    private static func stablePathHash(_ value: String) -> UInt64 {
        var hash: UInt64 = 1469598103934665603
        for byte in value.utf8 {
            hash ^= UInt64(byte)
            hash &*= 1099511628211
        }
        return hash
    }

    deinit {
        if Self.singletonLockFD >= 0 {
            close(Self.singletonLockFD)
            Self.singletonLockFD = -1
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        if shouldTerminateDuplicateInstance() {
            NSApp.terminate(nil)
            return
        }

        NSApp.setActivationPolicy(.accessory)
    }

    private func shouldTerminateDuplicateInstance() -> Bool {
        let currentBundlePath = Bundle.main.bundleURL.resolvingSymlinksInPath().path
        let lockPath = Self.singletonLockPath(for: currentBundlePath)

        // Try to acquire singleton lock with grace period for "Quit & Reopen" handoff
        let lockAcquired = acquireSingletonLock(lockPath: lockPath, timeoutSeconds: Self.relaunchGracePeriodSeconds)

        // Always check for other running instances to handle:
        // 1. Mixed-version scenarios (older builds not using lock protocol)
        // 2. Lock acquisition failures (fallback to process-based detection)
        return checkAndActivateDuplicateInstance(currentBundlePath: currentBundlePath, lockAcquired: lockAcquired)
    }

    private func checkAndActivateDuplicateInstance(currentBundlePath: String, lockAcquired: Bool) -> Bool {
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

    private func acquireSingletonLock(lockPath: String, timeoutSeconds: TimeInterval) -> Bool {
        if Self.singletonLockFD >= 0 {
            return true
        }

        let fd = open(lockPath, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
        // If we can't create the lock file, return false to trigger fallback process detection
        guard fd >= 0 else {
            return false
        }

        let deadline = Date().addingTimeInterval(timeoutSeconds)
        while true {
            if flock(fd, LOCK_EX | LOCK_NB) == 0 {
                Self.singletonLockFD = fd
                return true
            }

            if errno != EWOULDBLOCK && errno != EAGAIN {
                break
            }

            if Date() >= deadline {
                break
            }

            usleep(Self.lockPollIntervalMicros)
        }

        close(fd)
        return false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if let window = sender.windows.first {
            sender.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
        }
        NotificationCenter.default.post(name: .lookActivateLauncherRequested, object: nil)
        return true
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        DispatchQueue.main.async {
            if let app = notification.object as? NSApplication,
                let window = app.windows.first
            {
                app.activate(ignoringOtherApps: true)
                window.makeKeyAndOrderFront(nil)
            }
            NotificationCenter.default.post(name: .lookActivateLauncherRequested, object: nil)
        }
    }

    func applicationWillResignActive(_ notification: Notification) {
        NotificationCenter.default.post(name: .lookHideLauncherRequested, object: nil)
    }
}
