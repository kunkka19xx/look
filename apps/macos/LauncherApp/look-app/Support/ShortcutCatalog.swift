import Foundation

/// One documented shortcut.
///
/// `id` is the stable handle a binding hangs off: `ShortcutBindingTable` names
/// rows by id, so `keys` is only the default for a row that has one, and both
/// the help screen and Settings show the chord the user is actually on.
struct ShortcutEntry: Identifiable {
    let id: String
    let keys: String
    let action: String
    /// False when there is no chord to reassign - a typed prefix, a positional
    /// `Cmd+N` derived from catalog order, or a pointer affordance. Settings
    /// offers a recorder only where a binding exists, so it never presents a row
    /// that cannot be honoured.
    var remappable: Bool = true

    init(_ id: String, _ keys: String, _ action: String, remappable: Bool = true) {
        self.id = id
        self.keys = keys
        self.action = action
        self.remappable = remappable
    }
}

/// The filter capsules on the help screen. Settings renders every topic in this
/// order, so "all shortcuts" and "the Shortcuts tab" are the same list.
enum ShortcutTopic: String, CaseIterable, Identifiable {
    case main
    case ai
    case prefixes
    case command

    var id: String { rawValue }

    var label: String {
        switch self {
        case .main: return "Main"
        case .ai: return "AI"
        case .prefixes: return "Prefixes"
        case .command: return "Command"
        }
    }
}

/// Which slice of the catalog a surface is showing. `all` keeps one scroll; the
/// rest narrow it, so arriving from a mode lands on that mode's keys instead of
/// a page the reader has to search.
///
/// Both the help screen (`Cmd+H`) and Settings > Shortcuts filter by this, which
/// is why it sits with the catalog rather than with either screen.
enum ShortcutTopicFilter: CaseIterable, Identifiable {
    case all
    case topic(ShortcutTopic)

    static var allCases: [ShortcutTopicFilter] { [.all] + ShortcutTopic.allCases.map(Self.topic) }

    static let ai = ShortcutTopicFilter.topic(.ai)

    var id: String {
        switch self {
        case .all: return "all"
        case .topic(let topic): return topic.id
        }
    }

    var label: String {
        switch self {
        case .all: return "All"
        case .topic(let topic): return topic.label
        }
    }

    var groups: [ShortcutGroup] {
        switch self {
        case .all: return ShortcutCatalog.groups
        case .topic(let topic): return ShortcutCatalog.groups(for: topic)
        }
    }
}

extension ShortcutTopicFilter: Equatable {
    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

/// One titled block. The title is the identity: two groups never share one.
struct ShortcutGroup: Identifiable {
    let title: String
    let topic: ShortcutTopic
    let entries: [ShortcutEntry]
    var id: String { title }
}

/// The single source of truth for keyboard documentation. Every row carrying a
/// `ShortcutBindingTable` entry is rebindable in Settings > Shortcuts, where the
/// `keys` below are what Reset goes back to.
///
/// Both surfaces read this: the in-window help screen (`Cmd+H`, filtered by
/// topic) and Settings > Shortcuts (flat, every group). They used to be two
/// hand-maintained tables, which drifted - six rows were duplicated verbatim,
/// the AI keys existed only in help, and the `Cmd+N` command list in Settings
/// had gone stale enough to name the wrong command.
enum ShortcutCatalog {
    private static let staticGroups: [ShortcutGroup] = [
        ShortcutGroup(title: "Global", topic: .main, entries: [
            ShortcutEntry("global.toggleLauncher", "Cmd+Space", "Show or hide Look from any app"),
        ]),

        ShortcutGroup(title: "Main", topic: .main, entries: [
            ShortcutEntry("main.open", "Enter", "Open selected app/file/folder or copy selected clipboard item"),
            ShortcutEntry("main.copy", "Cmd+C", "Copy selected file/folder to pasteboard"),
            ShortcutEntry("main.pick", "Cmd+P", "Toggle pick on selected file/folder (multi-select copy)"),
            ShortcutEntry("main.openPicked", "Shift+Enter", "Open all picked files/folders at once"),
            ShortcutEntry("main.clearPicks", "Cmd+Shift+P", "Clear all picked items"),
            ShortcutEntry("main.trash", "Cmd+D", "Trash selected file/folder (Trash pin: empty it) or remove the clipboard item"),
            ShortcutEntry("main.actions", "Cmd+K / Ctrl+K", "Open the action menu for the selected row (Cmd+J/K or Ctrl+J/K move, Enter runs)"),
            ShortcutEntry("main.quickActionToggle", "Cmd+O", "Flip the selected row's toggle (Bluetooth, Wi-Fi, Do Not Disturb)"),
            ShortcutEntry("main.moveTab", "Tab / Shift+Tab", "Move selection"),
            ShortcutEntry("main.moveArrows", "Up / Down", "Move selection"),
            ShortcutEntry("main.reveal", "Cmd+F", "Reveal selected app/file/folder in Finder"),
            ShortcutEntry("main.edit", "Cmd+E", "Open selected file/folder in your editor (set text_editor / code_editor)"),
            ShortcutEntry("main.terminal", "Cmd+T", "Open a terminal there (set terminal); switches theme when no row is selected"),
            ShortcutEntry("main.webSearch", "Cmd+Enter", "Search current query on Google"),
            ShortcutEntry("main.commandMode", "Cmd+/", "Enter command mode"),
            ShortcutEntry("main.commandJump", ":cmd", "Jump to a command from home (e.g. :calc 2+2, :kill chrome)", remappable: false),
            ShortcutEntry("main.hideApp", "Cmd+Shift+H", "Hide the selected app from Look"),
            ShortcutEntry("main.help", "Cmd+H", "Toggle this help screen"),
            ShortcutEntry("main.back", "Esc", "Back / close (context dependent)"),
            ShortcutEntry("main.hideLauncher", "Shift+Esc", "Hide launcher"),
        ]),

        // The strip on the empty home screen. Keys are the tile mnemonics from
        // the shared catalog (core/qactions), fired with Cmd.
        ShortcutGroup(title: "Super actions", topic: .main, entries: [
            ShortcutEntry("super.bluetoothWifi", "Cmd+B / Cmd+W", "Toggle Bluetooth / Wi-Fi"),
            ShortcutEntry("super.themeAwake", "Cmd+T / Cmd+K", "Switch theme / toggle Keep Awake (empty query only; Cmd+T opens a terminal once a row is selected)"),
            ShortcutEntry("super.screensaverMic", "Cmd+S / Cmd+M", "Start screensaver / mute mic"),
            ShortcutEntry("super.playPause", "Cmd+P", "Play/pause the current track"),
            ShortcutEntry("super.power", "Cmd+R / Cmd+D", "Restart / Shut Down (press twice, Esc cancels)"),
            ShortcutEntry("super.toggleStrip", "Settings > Appearance", "Show or hide the super actions strip", remappable: false),
        ]),

        ShortcutGroup(title: "Clipboard history", topic: .main, entries: [
            ShortcutEntry("clipboard.copyBack", "Enter", "Copy selected history item back to clipboard"),
            ShortcutEntry("clipboard.paste", "Cmd+I", "Paste selected history item into the app you came from (text or image)"),
            ShortcutEntry("clipboard.remove", "Cmd+D", "Remove selected clipboard item from Look history"),
        ]),

        ShortcutGroup(title: "View & panels", topic: .main, entries: [
            ShortcutEntry("view.settings", "Cmd+Shift+,", "Open/close settings panel"),
            ShortcutEntry("view.reloadConfig", "Cmd+Shift+;", "Reload .look/config"),
            ShortcutEntry("view.toggleLayout", "Cmd+Shift+C", "Switch between split and compact until Look quits (config unchanged)"),
            ShortcutEntry("view.zoomOut", "Cmd+-", "Zoom UI scale out"),
            ShortcutEntry("view.zoomIn", "Cmd+=", "Zoom UI scale in"),
            ShortcutEntry("view.zoomReset", "Cmd+0", "Reset UI scale (opens the tenth session while the AI list is up)"),
        ]),

        // The `>` assistant: the sessions list, a live conversation, and the keys
        // that only exist there (the running-apps strip is hidden in this mode,
        // so Cmd+digit addresses conversations instead of apps).
        ShortcutGroup(title: "AI mode (>)", topic: .ai, entries: [
            ShortcutEntry("ai.enter", ">", "Enter AI mode (a dead-end Enter on the home screen goes here too)", remappable: false),
            ShortcutEntry("ai.send", "Enter", "Send the message, or open the highlighted conversation"),
            ShortcutEntry("ai.newline", "Shift+Enter", "New line in the message (the box grows to 6 lines)"),
            ShortcutEntry("ai.historyOlder", "Option+Up", "Walk back through your recent prompts, like a shell history"),
            ShortcutEntry("ai.historyNewer", "Option+Down", "Walk forward through your recent prompts"),
            ShortcutEntry("ai.selectText", "Shift+Up / Shift+Down", "Select text in the message you are composing"),
            ShortcutEntry("ai.openSession", "Cmd+1..Cmd+9, Cmd+0", "Open the conversation carrying that chip (Cmd+0 is the tenth)"),
            ShortcutEntry("ai.moveList", "Tab / Up / Down", "Move over the conversation list"),
            ShortcutEntry("ai.deleteSession", "Cmd+D", "Delete the highlighted conversation"),
            ShortcutEntry("ai.undo", "Cmd+Z", "Undo the last action, or restore a just-deleted conversation"),
            ShortcutEntry("ai.stop", "Cmd+.", "Stop a streaming answer"),
            ShortcutEntry("ai.mention", "@name", "Attach a file to the message (Enter picks the highlighted one)", remappable: false),
            ShortcutEntry("ai.exactTime", "@ 5pm", "Set an exact time on an event or reminder", remappable: false),
            ShortcutEntry("ai.chooseNumbered", "1, 2, 3 + Enter", "Answer a \u{201C}which one?\u{201D} list", remappable: false),
            ShortcutEntry("ai.help", "Cmd+H", "Open this help without leaving the conversation"),
            ShortcutEntry("ai.escape", "Esc", "Close the file popup, then leave the conversation"),
            ShortcutEntry("ai.leave", "Shift+Esc", "Leave AI mode straight to the home screen"),
        ]),

        ShortcutGroup(title: "Query prefixes", topic: .prefixes, entries: prefixEntries),

        ShortcutGroup(title: "Command mode", topic: .command, entries: [commandSwitchEntry] + [
            ShortcutEntry("command.switchTab", "Tab / Shift+Tab", "Switch command"),
            ShortcutEntry("command.byPort", "3000", "Find process by port or PID", remappable: false),
            ShortcutEntry("command.killSelect", "Up / Down", "Select app in kill results"),
            ShortcutEntry("command.killConfirm", "Y / N", "Confirm/cancel kill action"),
            ShortcutEntry("command.back", "Esc", "Back to the app list"),
        ]),

        ShortcutGroup(title: "Pomodoro (/pomo)", topic: .command, entries: [
            ShortcutEntry("pomo.startPause", "Space", "Start / pause the active session"),
            ShortcutEntry("pomo.reset", "R", "Reset the timer back to idle"),
            ShortcutEntry("pomo.music", "P", "Toggle music play / pause"),
            ShortcutEntry("pomo.standby", "Mouse / key idle", "After 5s, panel fades to clock-only standby; any input restores", remappable: false),
            ShortcutEntry("pomo.menuBar", "Menu bar item", "Click the timer icon in the menu bar to jump back into /pomo", remappable: false),
        ]),

        ShortcutGroup(title: "Todo & Speed panels", topic: .command, entries: [
            ShortcutEntry("todo.togglePage", "Cmd+N", "Switch the Tasks / Stats page inside /todo"),
            ShortcutEntry("todo.save", "Cmd+S", "Save changes inside /todo"),
            ShortcutEntry("todo.undo", "Cmd+Z", "Undo the last task change inside /todo"),
            ShortcutEntry("todo.redo", "Cmd+Shift+Z", "Redo an undone task change inside /todo"),
            ShortcutEntry("speed.rerun", "R", "Run the test again inside /speed"),
            ShortcutEntry("speed.revealAddress", "E", "Show or hide the public address inside /speed"),
        ]),
    ]

    static var groups: [ShortcutGroup] {
        var all = staticGroups
        let appHotkeys = EngineBridge.shared.appHotkeys()
        if !appHotkeys.isEmpty {
            let entries = appHotkeys.map { spec in
                ShortcutEntry(
                    "app.\(spec.name)",
                    spec.display,
                    "Launch \(spec.target.isEmpty ? spec.name : spec.target)",
                    remappable: false
                )
            }
            all.insert(ShortcutGroup(title: "Applications", topic: .main, entries: entries), at: 1)
        }
        return all
    }

    /// Groups for one topic, in reading order.
    static func groups(for topic: ShortcutTopic) -> [ShortcutGroup] {
        groups.filter { $0.topic == topic }
    }

    static var allEntries: [ShortcutEntry] { groups.flatMap(\.entries) }

    static func entry(_ id: String) -> ShortcutEntry? {
        allEntries.first { $0.id == id }
    }

    /// Derived from the canonical prefix list so the help screen, the Shortcuts
    /// tab, and the `"` discovery menu cannot drift.
    private static var prefixEntries: [ShortcutEntry] {
        AppConstants.Launcher.PrefixSuggestion.all.map { entry in
            ShortcutEntry("prefix.\(entry.prefix)", entry.displayWithArg, entry.description, remappable: false)
        }
    }

    /// Derived from `commandCatalog`, whose ORDER is the mapping: ⌘N selects the
    /// Nth entry. Writing this list by hand is what let Settings claim ⌘4 was
    /// `/kill` after `/speed` was inserted ahead of it. Not remappable for the
    /// same reason - the binding is positional, so reordering the catalog is the
    /// only way to change it.
    private static var commandSwitchEntry: ShortcutEntry {
        let commands = AppConstants.Launcher.commandCatalog
        let names = commands.map { "/\($0.id)" }.joined(separator: ", ")
        let keys = commands.isEmpty ? "Cmd+1" : "Cmd+1..\(commands.count)"
        return ShortcutEntry("command.switchByIndex", keys, "Switch directly to \(names)", remappable: false)
    }
}
