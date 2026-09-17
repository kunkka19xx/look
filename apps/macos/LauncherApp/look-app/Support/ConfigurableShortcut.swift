import Foundation

/// A shortcut the user can rebind in Settings > Shortcuts.
///
/// It names the `ShortcutCatalog` entry it replaces and the config key that
/// stores it. Making another shortcut configurable is a new `Kind`, an entry in
/// `all`, and its case in `ConfigurableShortcut+Registration.swift`; the
/// settings row, the recorder and the save path already work from this list.
struct ConfigurableShortcut: Identifiable {
    enum Kind {
        case launcherToggle
    }

    let kind: Kind
    let catalogID: String
    let configKey: String

    var id: String { catalogID }

    static let launcherToggle = ConfigurableShortcut(
        kind: .launcherToggle,
        catalogID: "global.toggleLauncher",
        configKey: "launcher_hotkey"
    )

    static let all: [ConfigurableShortcut] = [.launcherToggle]

    static func forEntry(_ entryID: String) -> ConfigurableShortcut? {
        all.first { $0.catalogID == entryID }
    }

    static func forConfigKey(_ key: String) -> ConfigurableShortcut? {
        all.first { $0.configKey == key }
    }
}

/// Set while a shortcut recorder is listening. Every other key monitor passes
/// events through untouched meanwhile, so the recorder sees the press whatever
/// order AppKit runs the monitors in.
enum ShortcutCapture {
    static var isActive = false
}
