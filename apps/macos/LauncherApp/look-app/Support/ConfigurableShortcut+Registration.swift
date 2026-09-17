import Foundation

/// How each configurable shortcut reaches the code that registers it.
extension ConfigurableShortcut {
    /// The binding in effect now, shown until the user records another.
    var currentDisplay: String {
        switch kind {
        case .launcherToggle: LauncherHotkeyController.shared.display
        }
    }

    /// The config spelling "Reset" writes, or nil when it is not known yet.
    var defaultSpec: String? {
        switch kind {
        case .launcherToggle: LauncherHotkeyController.shared.defaultSpec
        }
    }

    /// How the binding waiting in `bindings` reads, or nil when there is none.
    func pendingDisplay(in bindings: [String: String]) -> String? {
        guard let spec = bindings[configKey], !spec.isEmpty else {
            return nil
        }
        return EngineBridge.shared.hotkeyCheck(spec)?.display
    }

    /// True when `bindings` holds a value that Save Config has not applied yet.
    func hasUnsavedChange(in bindings: [String: String]) -> Bool {
        guard let pending = pendingDisplay(in: bindings) else { return false }
        return pending != currentDisplay
    }

    /// Releases the key while a recorder listens, so pressing it is recorded
    /// instead of acted on.
    func suspend() {
        switch kind {
        case .launcherToggle: LauncherHotkeyController.shared.suspend()
        }
    }

    /// Registers whatever the config names again.
    func resume() {
        switch kind {
        case .launcherToggle: LauncherHotkeyController.shared.reload()
        }
    }
}
