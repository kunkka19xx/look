import AppKit
import Foundation

@MainActor
final class LauncherWindowCoordinator {
    private var focusRequestToken: UInt64 = 0

    func focusActiveInput(
        isSettingsVisible: @escaping () -> Bool,
        requestSettingsFocus: () -> Void,
        onFocus: @escaping () -> Void
    ) {
        if isSettingsVisible() {
            requestSettingsFocus()
            return
        }

        focusRequestToken &+= 1
        let token = focusRequestToken

        NSApplication.shared.activate(ignoringOtherApps: true)
        scheduleFocusRecovery(
            delays: [0.0, 0.04, 0.10],
            token: token,
            isSettingsVisible: isSettingsVisible,
            onFocus: onFocus
        )
    }

    func toggleWindowVisibility(onHide: () -> Void, onShow: () -> Void) {
        guard let window = launcherWindow() else { return }

        if window.isVisible && NSApplication.shared.isActive {
            onHide()
            return
        }

        NSApplication.shared.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        onShow()
    }

    func hideLauncherWindow(onHidden: () -> Void) {
        guard let window = launcherWindow() else { return }
        window.orderOut(nil)
        onHidden()
    }

    func isLauncherVisibleAndAppActive() -> Bool {
        let isVisible = launcherWindow()?.isVisible ?? false
        return NSApplication.shared.isActive && isVisible
    }

    private func scheduleFocusRecovery(
        delays: [Double],
        token: UInt64,
        isSettingsVisible: @escaping () -> Bool,
        onFocus: @escaping () -> Void
    ) {
        for delay in delays {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
                guard let self else { return }
                guard token == self.focusRequestToken else { return }
                guard !isSettingsVisible() else { return }
                guard let window = self.launcherWindow() else { return }

                if !window.isVisible {
                    window.makeKeyAndOrderFront(nil)
                } else {
                    window.makeKey()
                    window.orderFront(nil)
                }

                if let responder = self.findEditableTextField(in: window.contentView) {
                    window.makeFirstResponder(responder)
                }

                onFocus()
            }
        }
    }

    private func launcherWindow() -> NSWindow? {
        if let keyWindow = NSApplication.shared.keyWindow {
            return keyWindow
        }
        if let visibleWindow = NSApplication.shared.windows.first(where: { $0.isVisible }) {
            return visibleWindow
        }
        return NSApplication.shared.windows.first
    }

    private func findEditableTextField(in view: NSView?) -> NSView? {
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
}
