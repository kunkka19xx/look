import AppKit
import SwiftUI

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
        if let keyWindow = NSApplication.shared.keyWindow {
            return keyWindow
        }

        if let visibleWindow = NSApplication.shared.windows.first(where: { $0.isVisible }) {
            return visibleWindow
        }

        return NSApplication.shared.windows.first
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
        if let window = launcherWindow(), window.isVisible && NSApplication.shared.isActive {
            hideLauncherWindow()
            return
        }

        captureFrontmostAppForRestoreIfNeeded()
        _ = bridge.requestIndexRefresh()
        NSApplication.shared.unhide(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)

        if let window = launcherWindow() {
            window.makeKeyAndOrderFront(nil)
            activateLauncherModeAndFocus()
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
        guard let window = launcherWindow() else { return }
        focusRequestToken &+= 1
        isQueryFocused = false
        window.orderOut(nil)
        if restorePreviousApp {
            reactivatePreviouslyFocusedAppIfNeeded()
        } else {
            pidToRestoreOnHide = nil
        }
        refreshClipboardMonitoringMode()
    }

    func bringOpenedAppToFront(appBundlePath: String) {
        let appURL = URL(fileURLWithPath: appBundlePath)
        guard let bundle = Bundle(url: appURL),
              let bundleID = bundle.bundleIdentifier
        else {
            return
        }

        let ownPID = ProcessInfo.processInfo.processIdentifier
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.postOpenActivationDelay) {
            // Skip if the user has since switched to a different app — don't steal focus back.
            if let frontmost = NSWorkspace.shared.frontmostApplication,
               frontmost.processIdentifier != ownPID,
               frontmost.bundleIdentifier != bundleID {
                return
            }
            let candidates = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID)
            if let app = candidates.first(where: { !$0.isTerminated }) ?? candidates.first {
                _ = app.activate()
            }
        }
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

    func reactivatePreviouslyFocusedAppIfNeeded() {
        guard let pid = pidToRestoreOnHide else { return }
        pidToRestoreOnHide = nil
        guard pid != ProcessInfo.processInfo.processIdentifier else { return }
        guard let app = NSRunningApplication(processIdentifier: pid) else { return }
        guard !app.isTerminated else { return }

        DispatchQueue.main.asyncAfter(deadline: .now() + Self.postHideActivationDelay) {
            _ = app.activate()
        }
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
