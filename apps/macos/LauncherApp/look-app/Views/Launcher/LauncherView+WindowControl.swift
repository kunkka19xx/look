import AppKit
import OSLog
import SwiftUI

private let hotkeyLog = Logger(subsystem: "noah-code.Look", category: "hotkey")

extension LauncherView {
    func focusActiveInput(
        recoveryDelays: [Double] = [0.0, 0.04, 0.10],
        activateApp: Bool = true
    ) {
        if appUIState.showsThemeSettings {
            NotificationCenter.default.post(name: .lookFocusSettingsInputRequested, object: nil)
            return
        }

        focusRequestToken &+= 1
        let token = focusRequestToken

        if activateApp {
            NSApplication.shared.activate(ignoringOtherApps: true)
        }
        scheduleFocusRecovery(delays: recoveryDelays, token: token)
    }

    func activateLauncherModeAndFocus() {
        if appUIState.showsThemeSettings {
            appUIState.showsThemeSettings = false
        }

        if isCommandMode {
            pendingKillCandidate = nil
            if activeCommandAcceptsInput {
                focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
            } else {
                isQueryFocused = false
            }
            return
        }

        focusActiveInput()
    }

    func scheduleFocusRecovery(delays: [Double], token: UInt64) {
        for delay in delays {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                guard token == focusRequestToken else { return }
                guard !appUIState.showsThemeSettings else { return }
                guard let window = launcherWindow() else { return }

                if !window.isVisible {
                    window.makeKeyAndOrderFront(nil)
                } else {
                    window.makeKey()
                    window.orderFront(nil)
                }

                if let responder = findEditableTextField(in: window.contentView) {
                    window.makeFirstResponder(responder)
                }

                isQueryFocused = true
            }
        }
    }

    func launcherWindow() -> NSWindow? {
        // The app has multiple NSWindows now (the launcher itself, the
        // menu-bar status item button window, the pomo popover anchor).
        // The status item / popover windows are tiny (≈16x24); the
        // launcher's minimum frame is 620x600 (set on ContentView). Use
        // a size threshold to filter them out.
        let isLauncherSized: (NSWindow) -> Bool = { w in
            w.frame.width >= 400 && w.frame.height >= 400
        }

        if let key = NSApplication.shared.keyWindow, isLauncherSized(key) {
            return key
        }

        let windows = NSApplication.shared.windows

        if let visibleLauncher = windows.first(where: { $0.isVisible && isLauncherSized($0) }) {
            return visibleLauncher
        }

        if let anyLauncher = windows.first(where: isLauncherSized) {
            return anyLauncher
        }

        // Fallbacks if for some reason no launcher-sized window exists yet.
        if let key = NSApplication.shared.keyWindow { return key }
        if let visible = windows.first(where: { $0.isVisible }) { return visible }
        return windows.first
    }

    func findEditableTextField(in view: NSView?) -> NSView? {
        guard let view else { return nil }

        if let textField = view as? NSTextField,
            textField.isEditable,
            !textField.isHidden,
            textField.alphaValue > 0.01
        {
            return textField
        }

        for subview in view.subviews {
            if let found = findEditableTextField(in: subview) {
                return found
            }
        }

        return nil
    }

    func toggleWindowVisibility() {
        let win = launcherWindow()
        let isActive = NSApplication.shared.isActive
        let visibleWindowCount = NSApplication.shared.windows.filter { $0.isVisible }.count
        hotkeyLog.notice("toggle: isActive=\(isActive) windowCount=\(NSApplication.shared.windows.count) visibleCount=\(visibleWindowCount) keyWindow=\(NSApplication.shared.keyWindow != nil) winIsVisible=\(win?.isVisible ?? false) winIsHidden=\(NSApp.isHidden)")

        if let window = win, window.isVisible && isActive {
            hotkeyLog.notice("toggle: -> HIDE branch")
            hideLauncherWindow()
            return
        }

        hotkeyLog.notice("toggle: -> SHOW branch")
        captureFrontmostAppForRestoreIfNeeded()
        _ = bridge.requestIndexRefresh()
        // Warm the on-device model the instant the launcher opens so the first
        // AI answer doesn't pay the cold-load cost while the user types.
        if themeStore.settings.aiEnabled {
            AIQueryRouter.shared.prewarm(themeStore.settings.aiProvider)
        }
        NSApplication.shared.unhide(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)

        if let window = launcherWindow() {
            window.makeKeyAndOrderFront(nil)
            activateLauncherModeAndFocus()
            let frameStr = NSStringFromRect(window.frame)
            hotkeyLog.notice("toggle: SHOW done - visible=\(window.isVisible) onActiveSpace=\(window.isOnActiveSpace) frame=\(frameStr, privacy: .public)")
            return
        }

        openWindow(id: "main")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            NSApplication.shared.activate(ignoringOtherApps: true)
            launcherWindow()?.makeKeyAndOrderFront(nil)
            activateLauncherModeAndFocus()
        }
    }

    func hideLauncherWindow(restorePreviousApp: Bool = true) {
        guard let window = launcherWindow() else {
            hotkeyLog.notice("hide: no window")
            return
        }
        focusRequestToken &+= 1
        isQueryFocused = false
        // Don't leave a stale Empty Trash confirmation to reappear on next show.
        pendingEmptyTrashCount = nil
        let wasVisible = window.isVisible
        window.orderOut(nil)
        hotkeyLog.notice("hide: orderOut wasVisible=\(wasVisible) restore=\(restorePreviousApp)")

        if restorePreviousApp {
            _ = reactivatePreviouslyFocusedAppIfNeeded()
        } else {
            pidToRestoreOnHide = nil
        }

        refreshClipboardMonitoringMode()
    }

    func captureFrontmostAppForRestoreIfNeeded() {
        guard let frontmost = NSWorkspace.shared.frontmostApplication else {
            pidToRestoreOnHide = nil
            return
        }

        if frontmost.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            pidToRestoreOnHide = nil
            return
        }

        pidToRestoreOnHide = frontmost.processIdentifier
    }

    @discardableResult
    func reactivatePreviouslyFocusedAppIfNeeded() -> Bool {
        guard let pid = pidToRestoreOnHide else { return false }
        pidToRestoreOnHide = nil
        guard pid != ProcessInfo.processInfo.processIdentifier else { return false }
        guard let app = NSRunningApplication(processIdentifier: pid) else { return false }
        guard !app.isTerminated else { return false }

        DispatchQueue.main.asyncAfter(deadline: .now() + Self.postHideActivationDelay) {
            _ = app.activate()
        }
        return true
    }

    func refreshClipboardMonitoringMode() {
        let isVisible = launcherWindow()?.isVisible ?? false
        if NSApplication.shared.isActive && isVisible {
            clipboardStore.setMonitoringMode(.foreground)
        } else {
            clipboardStore.setMonitoringMode(.background)
        }
    }
}
