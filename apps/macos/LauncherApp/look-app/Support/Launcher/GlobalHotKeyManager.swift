import AppKit
import Carbon
import OSLog

nonisolated private let hotkeyLog = Logger(subsystem: "noah-code.Look", category: "hotkey")

@MainActor
final class GlobalHotKeyManager {
    private struct AppHotKeyRegistration {
        let id: UInt32
        let hotkey: CarbonHotkey
        let target: String
    }

    // nonisolated(unsafe) so the nonisolated deinit can release these
    // (they're Carbon/AppKit handles - not actually Sendable, but they're
    // only mutated from MainActor anyway).
    nonisolated(unsafe) private var hotKeyRef: EventHotKeyRef?
    nonisolated(unsafe) private var appHotKeyRefs: [EventHotKeyRef] = []
    nonisolated(unsafe) private var appHotKeys: [UInt32: AppHotKeyRegistration] = [:]
    nonisolated(unsafe) private var eventHandler: EventHandlerRef?
    // Carbon's RegisterEventHotKey only fires when the registering app is
    // NOT the currently-active app. When Look is in the foreground (e.g.
    // user has the launcher open and focused), Cmd+Space goes through
    // the normal local event chain instead. Install a parallel local
    // NSEvent monitor so the toggle works regardless of focus state.
    nonisolated(unsafe) private var localMonitor: Any?

    private var retryAttempts = 0
    private static let maxRetryAttempts = 5
    private var retryWorkItem: DispatchWorkItem?
    private var hotkey = CarbonHotkey.fallback
    // Reserve the toggle chord even while Carbon registration is being retried.
    private var toggleReserved = false

    static weak var current: GlobalHotKeyManager?

    init() {
        Self.current = self
    }

    // deinit is nonisolated; unregister is MainActor. Inline the cleanup
    // here using nonisolated-safe API only.
    deinit {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
        }
        for ref in appHotKeyRefs {
            UnregisterEventHotKey(ref)
        }
        if let eventHandler {
            RemoveEventHandler(eventHandler)
        }
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
        }
    }

    /// `noErr`, or the Carbon status that made the hotkey dead. Retries keep
    /// running either way.
    @discardableResult
    func registerToggleHotKey(_ hotkey: CarbonHotkey) -> OSStatus {
        self.hotkey = hotkey
        toggleReserved = true
        retryAttempts = 0
        return registerCurrentHotKey()
    }

    func registerAppHotKeys(_ items: [(hotkey: CarbonHotkey, spec: AppHotkeySpec)]) -> [AppHotkeySpec] {
        unregisterAppHotKeys()
        guard !items.isEmpty else { return [] }
        var registered: [AppHotkeySpec] = []
        ensureEventHandlerInstalled()
        var nextId: UInt32 = 2
        for item in items {
            if toggleReserved,
                item.hotkey.keyCode == hotkey.keyCode,
                item.hotkey.carbonModifiers == hotkey.carbonModifiers
            {
                hotkeyLog.error("Skipping app hotkey for \(item.spec.target): chord reserved for launcher toggle")
                continue
            }
            var ref: EventHotKeyRef?
            let hotKeyId = EventHotKeyID(signature: fourCharCode("LOOK"), id: nextId)
            let status = RegisterEventHotKey(
                item.hotkey.keyCode,
                item.hotkey.carbonModifiers,
                hotKeyId,
                GetEventDispatcherTarget(),
                0,
                &ref
            )
            if status == noErr, let ref {
                registered.append(item.spec)
                appHotKeyRefs.append(ref)
                appHotKeys[nextId] = AppHotKeyRegistration(
                    id: nextId,
                    hotkey: item.hotkey,
                    target: item.spec.target
                )
                hotkeyLog.notice("Registered app hotkey id=\(nextId) for \(item.spec.target)")
            } else {
                hotkeyLog.error("RegisterEventHotKey for \(item.spec.target) failed status=\(status)")
            }
            nextId += 1
        }
        installLocalMonitor()
        return registered
    }

    func unregisterAppHotKeys() {
        for ref in appHotKeyRefs {
            UnregisterEventHotKey(ref)
        }
        appHotKeyRefs.removeAll()
        appHotKeys.removeAll()
        cleanUpEventHandlerIfEmpty()
        installLocalMonitor()
    }

    func handleAppHotKey(id: UInt32) {
        guard let item = appHotKeys[id] else {
            hotkeyLog.error("No registered app hotkey for id=\(id)")
            return
        }
        hotkeyLog.notice("Firing app hotkey for \(item.target)")
        launchTargetApp(item.target)
    }

    private func launchTargetApp(_ target: String) {
        NotificationCenter.default.post(name: .lookHideLauncherRequested, object: nil)
        if let bundlePath = AppBundleLocator.bundlePath(forAppNamed: target) {
            let url = URL(fileURLWithPath: bundlePath)
            let config = NSWorkspace.OpenConfiguration()
            config.activates = true
            NSWorkspace.shared.openApplication(at: url, configuration: config) { _, error in
                if let error {
                    hotkeyLog.error("openApplication failed for \(bundlePath): \(error.localizedDescription)")
                }
            }
        } else if target.hasPrefix("/") || target.hasPrefix("~") {
            let expanded = (target as NSString).expandingTildeInPath
            let url = URL(fileURLWithPath: expanded)
            NSWorkspace.shared.open(url)
        } else {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
            process.arguments = ["-a", target]
            do {
                try process.run()
            } catch {
                hotkeyLog.error("open -a failed for \(target): \(error.localizedDescription)")
            }
        }
    }

    private func ensureEventHandlerInstalled() {
        guard eventHandler == nil else { return }
        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        let status = InstallEventHandler(
            GetEventDispatcherTarget(),
            { _, event, _ in
                var hotKeyId = EventHotKeyID()
                let status = GetEventParameter(
                    event,
                    EventParamName(kEventParamDirectObject),
                    EventParamType(typeEventHotKeyID),
                    nil,
                    MemoryLayout<EventHotKeyID>.size,
                    nil,
                    &hotKeyId
                )
                guard status == noErr else { return noErr }

                if hotKeyId.signature == fourCharCode("LOOK") {
                    let id = hotKeyId.id
                    if id == 1 {
                        hotkeyLog.notice("CARBON hotkey fired: toggle (app active=\(NSApp.isActive))")
                        DispatchQueue.main.async {
                            NotificationCenter.default.post(name: .lookToggleWindowRequested, object: nil)
                        }
                    } else {
                        hotkeyLog.notice("CARBON app hotkey fired: id=\(id) (app active=\(NSApp.isActive))")
                        DispatchQueue.main.async {
                            GlobalHotKeyManager.current?.handleAppHotKey(id: id)
                        }
                    }
                }
                return noErr
            },
            1,
            &eventType,
            nil,
            &eventHandler
        )
        hotkeyLog.notice("InstallEventHandler status=\(status)")
    }

    @discardableResult
    private func registerCurrentHotKey() -> OSStatus {
        retryWorkItem?.cancel()
        retryWorkItem = nil
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }

        let hotKeyId = EventHotKeyID(signature: fourCharCode("LOOK"), id: 1)
        let registerStatus = RegisterEventHotKey(
            hotkey.keyCode,
            hotkey.carbonModifiers,
            hotKeyId,
            GetEventDispatcherTarget(),
            0,
            &hotKeyRef
        )
        hotkeyLog.notice("RegisterEventHotKey status=\(registerStatus) (noErr=0; -9878=hotkey already in use)")

        if registerStatus == noErr {
            ensureEventHandlerInstalled()
            retryAttempts = 0
        } else {
            scheduleRetry()
        }

        installLocalMonitor()
        return registerStatus
    }

    private func installLocalMonitor() {
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
            self.localMonitor = nil
        }
        guard hotKeyRef != nil || !appHotKeys.isEmpty else { return }
        let hotkey = hotkey
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, !ShortcutCapture.isActive else { return event }
            if self.hotKeyRef != nil && hotkey.matches(event) {
                hotkeyLog.notice("LOCAL monitor fired (app active=\(NSApp.isActive))")
                NotificationCenter.default.post(name: .lookToggleWindowRequested, object: nil)
                return nil   // consume - don't let any field eat the space
            }
            for item in self.appHotKeys.values {
                if item.hotkey.matches(event) {
                    hotkeyLog.notice("LOCAL monitor fired for app: \(item.target)")
                    self.launchTargetApp(item.target)
                    return nil
                }
            }
            return event
        }
    }

    private func scheduleRetry() {
        guard retryAttempts < Self.maxRetryAttempts else {
            hotkeyLog.error("RegisterEventHotKey still failing after \(Self.maxRetryAttempts) attempts; giving up on global hotkey")
            return
        }
        retryAttempts += 1
        let delay = min(3.0, 0.5 * Double(retryAttempts))
        hotkeyLog.notice("scheduling hotkey re-registration attempt \(self.retryAttempts) in \(delay)s")
        let work = DispatchWorkItem { [weak self] in
            self?.registerCurrentHotKey()
        }
        retryWorkItem = work
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }

    private func cleanUpEventHandlerIfEmpty() {
        if hotKeyRef == nil && appHotKeyRefs.isEmpty {
            if let eventHandler {
                RemoveEventHandler(eventHandler)
                self.eventHandler = nil
            }
        }
    }

    func unregisterToggleHotKey() {
        toggleReserved = false
        retryWorkItem?.cancel()
        retryWorkItem = nil
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }
        cleanUpEventHandlerIfEmpty()
        installLocalMonitor()
    }

    func suspend() {
        retryWorkItem?.cancel()
        retryWorkItem = nil
        unregisterToggleHotKey()
        unregisterAppHotKeys()
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
            self.localMonitor = nil
        }
    }

    func unregister() {
        unregisterToggleHotKey()
    }
}

private func fourCharCode(_ text: String) -> OSType {
    text.utf8.reduce(0) { ($0 << 8) + OSType($1) }
}
