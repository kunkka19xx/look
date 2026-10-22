import Foundation

/// What a rebindable shortcut needs from the code that registers it with the
/// system. Only the launcher hotkey does: an in-app chord is matched by monitors
/// that already stand down while `ShortcutCapture` is listening.
@MainActor
protocol ShortcutRegistration: AnyObject {
    var display: String { get }
    var defaultSpec: String? { get }
    func suspend()
    @discardableResult func reload() -> String?
}

/// A shortcut the user can rebind in Settings > Shortcuts: the global toggle,
/// plus every in-app chord in `ShortcutBindingTable`. The recorder, the load and
/// the save paths all work from `all`.
struct ConfigurableShortcut {
    let catalogID: String
    let configKey: String
    let registration: ShortcutRegistration?
    let local: LocalShortcut?

    @MainActor
    static let all: [ConfigurableShortcut] =
        [
            ConfigurableShortcut(
                catalogID: "global.toggleLauncher",
                configKey: LauncherHotkeyConfig.key,
                registration: LauncherHotkeyController.shared,
                local: nil)
        ]
        + ShortcutBindingTable.all.map {
            ConfigurableShortcut(
                catalogID: $0.catalogID, configKey: $0.configKey, registration: nil,
                local: $0.shortcut)
        }

    /// Mirrors count: the other rows ⌘D documents render this same recorder.
    @MainActor
    static func forEntry(_ entryID: String) -> ConfigurableShortcut? {
        all.first { shortcut in
            guard let local = shortcut.local else { return shortcut.catalogID == entryID }
            return ShortcutBindingTable.definition(local).documents(entryID)
        }
    }

    @MainActor
    static func forConfigKey(_ key: String) -> ConfigurableShortcut? {
        all.first { $0.configKey == key }
    }

    /// What the key capsule shows now, saved or not.
    @MainActor
    var display: String {
        if let registration { return registration.display }
        return local.map { ShortcutBindings.shared.display($0) } ?? catalogID
    }

    /// The value Reset would write, or nil when already on the default.
    @MainActor
    var resetSpec: String? {
        guard let registration else {
            return local.flatMap { ShortcutBindings.shared.resettableDefault($0) }
        }
        guard let spec = registration.defaultSpec, check(spec)?.display != registration.display
        else { return nil }
        return spec
    }

    /// In-app chords accept Shift+Enter and Shift+Esc; a global one does not.
    nonisolated func check(_ spec: String) -> HotkeyCheck? {
        local == nil
            ? EngineBridge.shared.hotkeyCheck(spec)
            : EngineBridge.shared.hotkeyCheckLocal(spec)
    }

    /// Why a just-captured chord cannot be used. A menu item draws its own key
    /// equivalent, and SwiftUI can only spell the keys it knows.
    @MainActor
    func unusableReason(_ check: HotkeyCheck) -> String? {
        guard let local, ShortcutBindingTable.definition(local).menuDriven,
            check.hotkey.map({ ShortcutBindings.menuKeyEquivalent($0.key) == nil }) ?? true
        else { return nil }
        return "A menu shortcut cannot use that key"
    }

    @MainActor
    func suspend() {
        registration?.suspend()
    }

    @MainActor
    func reload() {
        registration?.reload()
    }

    func pendingDisplay(in bindings: [String: String]) -> String? {
        guard let value = bindings[configKey], !value.isEmpty else { return nil }
        let displays = ShortcutBindings.specs(in: value).compactMap { check($0)?.display }
        return displays.isEmpty ? nil : displays.joined(separator: " / ")
    }

    /// True when the value is what the shortcut ships with, so saving leaves
    /// the config clean instead of restating a default.
    @MainActor
    func isDefaultValue(_ value: String) -> Bool {
        guard let local else { return false }
        let defaults = ShortcutBindingTable.definition(local).defaults.compactMap { check($0)?.spec }
        let given = ShortcutBindings.specs(in: value).compactMap { check($0)?.spec }
        return !given.isEmpty && given == defaults
    }

    @MainActor
    func hasUnsavedChange(in bindings: [String: String]) -> Bool {
        pendingDisplay(in: bindings).map { $0 != display } ?? false
    }
}

/// The one key not derived from a catalog id: it shipped before the
/// `shortcut_*` family existed and keeps its spelling.
enum LauncherHotkeyConfig {
    static let key = "launcher_hotkey"
}

/// Set while a recorder listens; other key monitors pass events through.
enum ShortcutCapture {
    static var isActive = false
}
