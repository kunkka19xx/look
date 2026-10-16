import AppKit
import Combine
import OSLog
import SwiftUI

nonisolated private let bindingsLog = Logger(subsystem: "noah-code.Look", category: "shortcuts")

/// A resolved chord: what to compare an event against, plus what a menu item
/// needs to draw its own key equivalent.
struct LocalChord {
    let hotkey: CarbonHotkey
    let key: LauncherHotkeySpec.Key
    let spec: String

    /// The keypad's Enter is the same key as the main one for a shortcut, which
    /// is what the hand-written branches assumed.
    func matches(_ event: NSEvent) -> Bool {
        if hotkey.matches(event) { return true }
        guard hotkey.keyCode == UInt32(AppConstants.Launcher.KeyCode.returnKey),
            event.keyCode == AppConstants.Launcher.KeyCode.keypadEnter
        else { return false }
        return event.modifierFlags.intersection(CarbonHotkey.modifierMask) == hotkey.eventModifiers
    }
}

/// Every in-app shortcut, resolved from `ShortcutBindingTable` defaults and the
/// `shortcut_*` keys in `~/.look/config`.
///
/// The key monitors ask here instead of comparing keyCodes inline, so one table
/// decides what a chord means and a rebind cannot miss a branch. Core owns the
/// grammar; a chord matches on the keyCode its key sits at on the current
/// layout, which is why the old branches accepted a keyCode or a character.
final class ShortcutBindings: ObservableObject {
    static let shared = ShortcutBindings()

    /// Bumped on every change, so SwiftUI redraws: the chords are read through
    /// methods, which `@Published` cannot track.
    @Published private(set) var revision = 0

    /// `|`, as the config file already separates lists with. Not a comma:
    /// `cmd+shift+,` is a real shortcut.
    static let specSeparator = " | "

    private var overrides: [String: String] = [:]
    private var resolved: [LocalShortcut: [LocalChord]] = [:]

    init() {
        resolve()
    }

    /// `bindings` is keyed by config key, as parsed from `~/.look/config`.
    func apply(_ bindings: [String: String]) {
        let mine = bindings.filter { key, value in
            !value.isEmpty && ShortcutBindingTable.definition(configKey: key) != nil
        }
        guard mine != overrides else { return }
        overrides = mine
        resolve()
        revision += 1
    }

    func matches(_ shortcut: LocalShortcut, _ event: NSEvent) -> Bool {
        chords(shortcut).contains { $0.matches(event) }
    }

    func chords(_ shortcut: LocalShortcut) -> [LocalChord] {
        resolved[shortcut] ?? []
    }

    /// What the key capsule shows: every chord the shortcut answers to.
    func display(_ shortcut: LocalShortcut) -> String {
        let shown = chords(shortcut).map(\.hotkey.display)
        return shown.isEmpty
            ? ShortcutCatalog.entry(shortcut.rawValue)?.keys ?? shortcut.rawValue
            : shown.joined(separator: " / ")
    }

    func specs(_ shortcut: LocalShortcut) -> [String] {
        chords(shortcut).map(\.spec)
    }

    /// The default as a config value, for Reset. Nil when already on it.
    func resettableDefault(_ shortcut: LocalShortcut) -> String? {
        let definition = ShortcutBindingTable.definition(shortcut)
        guard let canonical = Self.canonical(definition.defaults), canonical != specs(shortcut) else {
            return nil
        }
        return definition.defaults.joined(separator: Self.specSeparator)
    }

    /// A stored value is one chord, or several separated by `|`.
    nonisolated static func specs(in value: String) -> [String] {
        value
            .split(separator: "|")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    /// Conflicts as lines for Settings.
    var conflictLines: [String] {
        var specsByShortcut: [LocalShortcut: [String]] = [:]
        for shortcut in LocalShortcut.allCases {
            specsByShortcut[shortcut] = specs(shortcut)
        }
        return ShortcutBindingTable.conflicts(specs: specsByShortcut).map { conflict in
            let display = chords(conflict.shortcuts[0]).first { $0.spec == conflict.spec }?.hotkey.display
            let names = conflict.shortcuts.map { ShortcutCatalog.entry($0.rawValue)?.action ?? $0.rawValue }
            return "\(display ?? conflict.spec) is bound to \(names.joined(separator: " and "))"
        }
    }

    /// Nil for a chord SwiftUI cannot spell (an F-key); the recorder refuses
    /// those for menu-driven shortcuts, so a binding is never silently dead.
    func menuShortcut(_ shortcut: LocalShortcut) -> (key: KeyEquivalent, modifiers: EventModifiers)? {
        guard let chord = chords(shortcut).first, let key = Self.menuKeyEquivalent(chord.key) else {
            return nil
        }
        return (key, Self.eventModifiers(chord.hotkey.eventModifiers))
    }

    private func resolve() {
        resolved = Dictionary(
            uniqueKeysWithValues: ShortcutBindingTable.all.map { definition in
                let specs = overrides[definition.configKey].map(Self.specs(in:)) ?? definition.defaults
                var chords = specs.compactMap(Self.chord(for:))
                if chords.isEmpty, overrides[definition.configKey] != nil {
                    // A hand-edited config can name a chord core rejects; the
                    // default beats leaving the action unreachable.
                    chords = definition.defaults.compactMap(Self.chord(for:))
                }
                return (definition.shortcut, chords)
            })
    }

    private static func chord(for spec: String) -> LocalChord? {
        guard let check = EngineBridge.shared.hotkeyCheckLocal(spec),
            let parsed = check.hotkey,
            let display = check.display,
            let hotkey = CarbonHotkey(hotkey: parsed, display: display)
        else {
            bindingsLog.error("shortcut \(spec, privacy: .public) is not a chord Look can bind")
            return nil
        }
        return LocalChord(hotkey: hotkey, key: parsed.key, spec: check.spec)
    }

    private static func canonical(_ specs: [String]) -> [String]? {
        let checked = specs.compactMap { EngineBridge.shared.hotkeyCheckLocal($0)?.spec }
        return checked.count == specs.count ? checked : nil
    }

    static func menuKeyEquivalent(_ key: LauncherHotkeySpec.Key) -> KeyEquivalent? {
        if let character = key.character, let first = character.first {
            return KeyEquivalent(first)
        }
        switch key.code {
        case "Space": return KeyEquivalent(" ")
        case "Enter": return .return
        case "Tab": return .tab
        case "Escape": return .escape
        case "ArrowUp": return .upArrow
        case "ArrowDown": return .downArrow
        case "ArrowLeft": return .leftArrow
        case "ArrowRight": return .rightArrow
        default: return nil
        }
    }

    private static func eventModifiers(_ flags: NSEvent.ModifierFlags) -> EventModifiers {
        var modifiers: EventModifiers = []
        if flags.contains(.command) { modifiers.insert(.command) }
        if flags.contains(.control) { modifiers.insert(.control) }
        if flags.contains(.option) { modifiers.insert(.option) }
        if flags.contains(.shift) { modifiers.insert(.shift) }
        return modifiers
    }
}

/// A menu item whose key equivalent follows the user's binding.
private struct ConfiguredMenuShortcut: ViewModifier {
    @ObservedObject private var shortcuts = ShortcutBindings.shared

    let shortcut: LocalShortcut

    func body(content: Content) -> some View {
        content.keyboardShortcut(
            shortcuts.menuShortcut(shortcut).map { KeyboardShortcut($0.key, modifiers: $0.modifiers) })
    }
}

extension View {
    func configuredShortcut(_ shortcut: LocalShortcut) -> some View {
        modifier(ConfiguredMenuShortcut(shortcut: shortcut))
    }
}
