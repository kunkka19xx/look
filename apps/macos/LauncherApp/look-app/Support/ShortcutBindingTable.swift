import Foundation

/// A shortcut Look matches itself, while its own window has the keys. The raw
/// value is the `ShortcutCatalog` id, so the documented row and the binding are
/// one handle.
///
/// Only modifier chords live here: Enter, Esc, Tab, the arrows and the Y/N
/// confirmations mean different things in each mode, so they stay fixed.
enum LocalShortcut: String, CaseIterable {
    case mainCopy = "main.copy"
    case mainPick = "main.pick"
    case mainClearPicks = "main.clearPicks"
    case mainOpenPicked = "main.openPicked"
    case mainTrash = "main.trash"
    case mainActions = "main.actions"
    case mainReveal = "main.reveal"
    case mainEdit = "main.edit"
    case mainTerminal = "main.terminal"
    case mainWebSearch = "main.webSearch"
    case mainCommandMode = "main.commandMode"
    case mainHideApp = "main.hideApp"
    case mainHelp = "main.help"
    case mainHideLauncher = "main.hideLauncher"
    case mainQuickActionToggle = "main.quickActionToggle"
    case clipboardPaste = "clipboard.paste"
    case viewSettings = "view.settings"
    case viewReloadConfig = "view.reloadConfig"
    case viewToggleLayout = "view.toggleLayout"
    case viewZoomIn = "view.zoomIn"
    case viewZoomOut = "view.zoomOut"
    case viewZoomReset = "view.zoomReset"
    case aiHistoryOlder = "ai.historyOlder"
    case aiHistoryNewer = "ai.historyNewer"
    case aiUndo = "ai.undo"
    case aiStop = "ai.stop"
    case todoTogglePage = "todo.togglePage"
    case todoSave = "todo.save"
    case todoUndo = "todo.undo"
    case todoRedo = "todo.redo"
}

/// Where a chord is live, which decides whether two of them collide. `/todo`
/// runs its own monitor, so its ⌘Z is not the assistant's; a menu command is
/// answered wherever Look is, so it collides with everything.
enum ShortcutContext {
    case launcher
    case todo
    case appWide

    func overlaps(_ other: ShortcutContext) -> Bool {
        self == .appWide || other == .appWide || self == other
    }
}

/// One rebindable chord. `defaults` is a list because some shortcuts ship with
/// aliases - the action menu answers to ⌘K, ⌃K, ⌘J and ⌃J - stored as `a | b`.
/// Recording replaces the list, so what you pressed is what you get.
///
/// `mirrors` are the other catalog rows this one binding drives: ⌘D trashes a
/// file, drops a clipboard entry and deletes a conversation through one branch.
struct ShortcutDefinition {
    let shortcut: LocalShortcut
    let defaults: [String]
    let context: ShortcutContext
    let mirrors: [String]
    /// Answered by a menu item, so its chord has to be one SwiftUI can spell.
    let menuDriven: Bool

    init(
        _ shortcut: LocalShortcut,
        _ defaults: [String],
        _ context: ShortcutContext,
        mirrors: [String] = [],
        menuDriven: Bool = false
    ) {
        self.shortcut = shortcut
        self.defaults = defaults
        self.context = context
        self.mirrors = mirrors
        self.menuDriven = menuDriven
    }

    var catalogID: String { shortcut.rawValue }
    var configKey: String { ShortcutBindingTable.configKey(for: catalogID) }

    func documents(_ entryID: String) -> Bool {
        entryID == catalogID || mirrors.contains(entryID)
    }
}

struct ShortcutConflict {
    let spec: String
    let shortcuts: [LocalShortcut]
}

/// Every in-app binding and the config key it is stored under. Specs are
/// written as a person would type them; core normalises the spelling, so
/// nothing here depends on the canonical order.
enum ShortcutBindingTable {
    static let configKeyPrefix = "shortcut_"

    static let all: [ShortcutDefinition] = [
        ShortcutDefinition(.mainCopy, ["cmd+c"], .launcher),
        ShortcutDefinition(.mainPick, ["cmd+p"], .launcher),
        ShortcutDefinition(.mainClearPicks, ["cmd+shift+p"], .launcher),
        ShortcutDefinition(.mainOpenPicked, ["shift+enter"], .launcher),
        ShortcutDefinition(
            .mainTrash, ["cmd+d"], .launcher,
            mirrors: ["clipboard.remove", "ai.deleteSession"]),
        ShortcutDefinition(.mainActions, ["cmd+k", "ctrl+k", "cmd+j", "ctrl+j"], .launcher),
        ShortcutDefinition(.mainReveal, ["cmd+f"], .launcher),
        ShortcutDefinition(.mainEdit, ["cmd+e"], .launcher),
        ShortcutDefinition(.mainTerminal, ["cmd+t"], .launcher),
        ShortcutDefinition(.mainWebSearch, ["cmd+enter"], .launcher),
        // ⌘? is ⌘/ with Shift on a US layout, and the chord answers to both.
        ShortcutDefinition(.mainCommandMode, ["cmd+/", "cmd+shift+/"], .launcher),
        ShortcutDefinition(.mainHideApp, ["cmd+shift+h"], .launcher),
        ShortcutDefinition(.mainHelp, ["cmd+h"], .launcher, mirrors: ["ai.help"]),
        ShortcutDefinition(.mainHideLauncher, ["shift+esc"], .launcher, mirrors: ["ai.leave"]),
        ShortcutDefinition(.mainQuickActionToggle, ["cmd+o"], .launcher),
        ShortcutDefinition(.clipboardPaste, ["cmd+i"], .launcher),
        ShortcutDefinition(.viewSettings, ["cmd+shift+,"], .appWide, menuDriven: true),
        ShortcutDefinition(.viewReloadConfig, ["cmd+shift+;"], .appWide, menuDriven: true),
        ShortcutDefinition(.viewToggleLayout, ["cmd+shift+c"], .launcher),
        ShortcutDefinition(.viewZoomIn, ["cmd+="], .appWide, menuDriven: true),
        ShortcutDefinition(.viewZoomOut, ["cmd+-"], .appWide, menuDriven: true),
        ShortcutDefinition(.viewZoomReset, ["cmd+0"], .appWide, menuDriven: true),
        ShortcutDefinition(.aiHistoryOlder, ["option+up"], .launcher),
        ShortcutDefinition(.aiHistoryNewer, ["option+down"], .launcher),
        ShortcutDefinition(.aiUndo, ["cmd+z"], .launcher),
        ShortcutDefinition(.aiStop, ["cmd+."], .launcher),
        ShortcutDefinition(.todoTogglePage, ["cmd+n"], .todo),
        ShortcutDefinition(.todoSave, ["cmd+s"], .todo),
        ShortcutDefinition(.todoUndo, ["cmd+z"], .todo),
        ShortcutDefinition(.todoRedo, ["cmd+shift+z"], .todo),
    ]

    static func definition(_ shortcut: LocalShortcut) -> ShortcutDefinition {
        // Every case is listed above, and a test holds that true.
        all.first { $0.shortcut == shortcut } ?? all[0]
    }

    /// Matches mirrors too, so every documented row finds its binding.
    static func definition(catalogID: String) -> ShortcutDefinition? {
        all.first { $0.documents(catalogID) }
    }

    static func definition(configKey: String) -> ShortcutDefinition? {
        all.first { $0.configKey == configKey }
    }

    /// `main.clearPicks` -> `shortcut_main_clear_picks`: derived, so a binding
    /// cannot be stored under a name its row does not predict.
    static func configKey(for catalogID: String) -> String {
        configKeyPrefix
            + catalogID.reduce(into: "") { key, character in
                if character == "." {
                    key.append("_")
                } else if character.isUppercase {
                    key.append("_\(character.lowercased())")
                } else {
                    key.append(character)
                }
            }
    }

    static func isShortcutKey(_ key: String) -> Bool {
        key.hasPrefix(configKeyPrefix)
    }

    /// Chords claimed twice in the same place. Reported, not refused:
    /// shadowing a shortcut you do not use is a fair thing to want.
    static func conflicts(specs: [LocalShortcut: [String]]) -> [ShortcutConflict] {
        var owners: [String: [LocalShortcut]] = [:]
        for shortcut in LocalShortcut.allCases {
            for spec in specs[shortcut] ?? [] {
                owners[spec, default: []].append(shortcut)
            }
        }
        return owners.sorted { $0.key < $1.key }.compactMap { spec, shortcuts in
            let contexts = shortcuts.map { definition($0).context }
            let collides = contexts.indices.contains { outer in
                contexts.indices.contains { $0 != outer && contexts[outer].overlaps(contexts[$0]) }
            }
            return collides ? ShortcutConflict(spec: spec, shortcuts: shortcuts) : nil
        }
    }
}
