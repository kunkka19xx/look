import Foundation

/// What a rebindable shortcut needs from the code that registers it.
@MainActor
protocol ShortcutRegistration: AnyObject {
    var display: String { get }
    var defaultSpec: String? { get }
    func suspend()
    @discardableResult func reload() -> String?
}

/// A shortcut the user can rebind in Settings > Shortcuts. To add one, list it
/// in `all`; the recorder, load and save paths work from this list.
struct ConfigurableShortcut {
    let catalogID: String
    let configKey: String
    let registration: ShortcutRegistration

    static let all = [
        ConfigurableShortcut(
            catalogID: "global.toggleLauncher",
            configKey: "launcher_hotkey",
            registration: LauncherHotkeyController.shared),
    ]

    static func forEntry(_ entryID: String) -> ConfigurableShortcut? {
        all.first { $0.catalogID == entryID }
    }

    static func forConfigKey(_ key: String) -> ConfigurableShortcut? {
        all.first { $0.configKey == key }
    }

    func pendingDisplay(in bindings: [String: String]) -> String? {
        guard let spec = bindings[configKey], !spec.isEmpty else { return nil }
        return EngineBridge.shared.hotkeyCheck(spec)?.display
    }

    func hasUnsavedChange(in bindings: [String: String]) -> Bool {
        pendingDisplay(in: bindings).map { $0 != registration.display } ?? false
    }
}

/// Set while a recorder listens; other key monitors pass events through.
enum ShortcutCapture {
    static var isActive = false
}
