import AppKit
import Carbon
import OSLog

nonisolated private let hotkeyLog = Logger(subsystem: "noah-code.Look", category: "hotkey")

@MainActor
final class GlobalHotKeyManager {
    // nonisolated(unsafe) so the nonisolated deinit can release these
    // (they're Carbon/AppKit handles — not actually Sendable, but they're
    // only mutated from MainActor anyway).
    nonisolated(unsafe) private var hotKeyRef: EventHotKeyRef?
    nonisolated(unsafe) private var eventHandler: EventHandlerRef?
    // Carbon's RegisterEventHotKey only fires when the registering app is
    // NOT the currently-active app. When Look is in the foreground (e.g.
    // user has the launcher open and focused), Cmd+Space goes through
    // the normal local event chain instead. Install a parallel local
    // NSEvent monitor so the toggle works regardless of focus state.
    nonisolated(unsafe) private var localMonitor: Any?

    // deinit is nonisolated; unregister is MainActor. Inline the cleanup
    // here using nonisolated-safe API only.
    deinit {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
        }
        if let eventHandler {
            RemoveEventHandler(eventHandler)
        }
        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
        }
    }

    func registerToggleHotKey() {
        unregister()

        let hotKeyId = EventHotKeyID(signature: fourCharCode("LOOK"), id: 1)
        let modifiers = UInt32(cmdKey)
        let keyCode = UInt32(kVK_Space)

        let registerStatus = RegisterEventHotKey(
            keyCode,
            modifiers,
            hotKeyId,
            GetEventDispatcherTarget(),
            0,
            &hotKeyRef
        )
        hotkeyLog.notice("RegisterEventHotKey status=\(registerStatus) (noErr=0; -9878=hotkey already in use)")

        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        if registerStatus == noErr {
            InstallEventHandler(
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

                    if hotKeyId.signature == fourCharCode("LOOK"), hotKeyId.id == 1 {
                        hotkeyLog.notice("CARBON hotkey fired (app active=\(NSApp.isActive))")
                        DispatchQueue.main.async {
                            NotificationCenter.default.post(name: .lookToggleWindowRequested, object: nil)
                        }
                    }
                    return noErr
                },
                1,
                &eventType,
                nil,
                &eventHandler
            )
        }

        // Local monitor: foreground-focused complement to the global
        // Carbon hotkey. Posts the same notification so the rest of the
        // app doesn't need to know which path delivered the event.
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let plainCmd = event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command
            if plainCmd && event.keyCode == UInt16(kVK_Space) {
                hotkeyLog.notice("LOCAL monitor fired (app active=\(NSApp.isActive))")
                NotificationCenter.default.post(name: .lookToggleWindowRequested, object: nil)
                return nil   // consume — don't let any field eat the space
            }
            return event
        }
    }

    func unregister() {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }

        if let eventHandler {
            RemoveEventHandler(eventHandler)
            self.eventHandler = nil
        }

        if let localMonitor {
            NSEvent.removeMonitor(localMonitor)
            self.localMonitor = nil
        }
    }
}

private func fourCharCode(_ text: String) -> OSType {
    text.utf8.reduce(0) { ($0 << 8) + OSType($1) }
}
