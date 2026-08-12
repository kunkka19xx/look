import AppKit
import Foundation
import OSLog

private typealias KeyCode = AppConstants.Launcher.KeyCode

@MainActor
final class KeyboardSelectionMonitor {
    private var monitor: Any?
    private var isKillConfirmationActive: @MainActor () -> Bool = { false }
    nonisolated private static let logger = Logger(subsystem: "noah-code.Look", category: "ui-key")
    nonisolated private static let debugKeyLoggingEnabled: Bool = {
        let env = ProcessInfo.processInfo.environment
        let raw = env["LOOK_UI_DEBUG_EVENTS"] ?? env["LOOK_DEV_HINT"] ?? ""
        return ["1", "true", "yes", "on"].contains(
            raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    }()

    nonisolated private static func logKey(_ message: String) {
        guard Self.debugKeyLoggingEnabled else { return }
        Self.logger.notice("\(message, privacy: .public)")
    }

    func start(
        onNext: @escaping @MainActor () -> Void,
        onPrevious: @escaping @MainActor () -> Void,
        onArrowDown: (@MainActor () -> Void)? = nil,
        onArrowUp: (@MainActor () -> Void)? = nil,
        onRecallPrompt: (@MainActor (Bool) -> Bool)? = nil,
        onEnterCommandMode: @escaping @MainActor () -> Void,
        onExitCommandMode: @escaping @MainActor () -> Void,
        onHideLauncher: @escaping @MainActor () -> Void,
        inCommandMode: @escaping @MainActor () -> Bool,
        onWebSearch: @escaping @MainActor () -> Void,
        onRevealInFinder: @escaping @MainActor () -> Void,
        onCopySelection: @escaping @MainActor () -> Bool,
        onTogglePick: @escaping @MainActor () -> Void,
        onClearPicked: @escaping @MainActor () -> Void,
        onOpenAllPicked: @escaping @MainActor () -> Void = {},
        hasPickedItems: @escaping @MainActor () -> Bool = { false },
        onToggleHelp: @escaping @MainActor () -> Void,
        onDismissHelpIfVisible: @escaping @MainActor () -> Bool,
        onSelectCommandByIndex: @escaping @MainActor (Int) -> Void,
        onActivateRunningApp: @escaping @MainActor (Int) -> Bool = { _ in false },
        onActivateSession: @escaping @MainActor (Int) -> Bool = { _ in false },
        onEscapeHome: (@MainActor () -> Bool)? = nil,
        onConfirmKill: (@MainActor () -> Void)? = nil,
        onCancelKill: (@MainActor () -> Void)? = nil,
        killConfirmationActive: @escaping @MainActor () -> Bool = { false },
        onRequestDelete: (@MainActor () -> Void)? = nil,
        onConfirmDelete: (@MainActor () -> Void)? = nil,
        onCancelDelete: (@MainActor () -> Void)? = nil,
        deleteConfirmationActive: @escaping @MainActor () -> Bool = { false },
        onConfirmHideApp: (@MainActor () -> Void)? = nil,
        onCancelHideApp: (@MainActor () -> Void)? = nil,
        hideAppConfirmationActive: @escaping @MainActor () -> Bool = { false },
        onCancelAction: (@MainActor () -> Void)? = nil,
        actionConfirmationActive: @escaping @MainActor () -> Bool = { false },
        onUndoAction: (@MainActor () -> Bool)? = nil,
        onStopGeneration: (@MainActor () -> Bool)? = nil,
        onToggleQuickAction: (@MainActor () -> Void)? = nil,
        hasToggleQuickAction: @escaping @MainActor () -> Bool = { false },
        isLaunchpadActive: @escaping @MainActor () -> Bool = { false },
        onLaunchpadMnemonic: (@MainActor (Character) -> Bool)? = nil,
        onLaunchpadEscape: (@MainActor () -> Bool)? = nil,
        onHideSelectedApp: (@MainActor () -> Bool)? = nil
    ) {
        guard monitor == nil else { return }
        self.isKillConfirmationActive = killConfirmationActive

        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            Self.logKey(
                "down keyCode=\(event.keyCode) chars=\(event.charactersIgnoringModifiers ?? "") flagsRaw=\(flags.rawValue) inCommand=\(inCommandMode())"
            )

            // Modal: eats what it does not recognise so no shortcut can act on a
            // selection mid-hide. Cmd+Q is exempt, a prompt must not trap the user.
            if hideAppConfirmationActive() {
                let char = event.charactersIgnoringModifiers?.lowercased()
                if flags == [.command] && char == "q" {
                    return event
                }
                if event.keyCode == KeyCode.escape {
                    onCancelHideApp?()
                    return nil
                }
                if event.keyCode == KeyCode.returnKey || event.keyCode == KeyCode.keypadEnter {
                    onConfirmHideApp?()
                    return nil
                }
                if char == "y" {
                    onConfirmHideApp?()
                    return nil
                }
                if char == "n" {
                    onCancelHideApp?()
                    return nil
                }
                return nil
            }

            if flags.contains(.command)
                && !flags.contains(.control)
                && !flags.contains(.option)
                && (event.keyCode == KeyCode.slash
                    || event.charactersIgnoringModifiers == "/"
                    || event.charactersIgnoringModifiers == "?")
            {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                    onEnterCommandMode()
                }
                return nil
            }

            // Empty-state launchpad mnemonics (⌘B/⌘W/⌘T/...). Only fires when the
            // launchpad is on screen, so it never shadows the result-oriented
            // chords below (Cmd+F reveal, Cmd+P pick, etc.) once the user types.
            // Match on the typed character, not a keyCode, so it holds on
            // non-QWERTY layouts (same rationale as the Cmd+O handler).
            if flags == [.command],
                isLaunchpadActive(),
                let handler = onLaunchpadMnemonic,
                let character = event.charactersIgnoringModifiers?.first,
                handler(character)
            {
                return nil
            }

            // ⌘+home-row jumps to a listed conversation (keys under the fingers:
            // a s d f g h j k l → rows 1-9). Gated on "browsing the sessions
            // list", so ⌘S/⌘F/⌘H etc. keep their normal meaning everywhere else.
            if flags == [.command],
                let ch = event.charactersIgnoringModifiers?.lowercased().first,
                let index = "asdfghjkl".firstIndex(of: ch).map({ "asdfghjkl".distance(from: "asdfghjkl".startIndex, to: $0) }),
                onActivateSession(index)
            {
                return nil
            }

            if (event.keyCode == KeyCode.returnKey || event.keyCode == KeyCode.keypadEnter) && flags == [.command] {
                onWebSearch()
                return nil
            }

            if (event.keyCode == KeyCode.f || event.charactersIgnoringModifiers?.lowercased() == "f")
                && flags == [.command]
            {
                onRevealInFinder()
                return nil
            }

            if (event.keyCode == KeyCode.c || event.charactersIgnoringModifiers?.lowercased() == "c")
                && flags == [.command]
            {
                if onCopySelection() {
                    return nil
                }
                return event
            }

            // ⌘. stops a running generation (the macOS-standard cancel chord).
            // The handler gates itself, so it only fires while streaming.
            if event.charactersIgnoringModifiers == "." && flags == [.command] {
                if onStopGeneration?() == true {
                    return nil
                }
                return event
            }

            // Undo the last action (its result row is showing). The handler gates
            // itself, so Cmd+Z passes through to text-field undo otherwise.
            if event.charactersIgnoringModifiers?.lowercased() == "z" && flags == [.command] {
                if onUndoAction?() == true {
                    return nil
                }
                return event
            }

            // The handler owns the gating, so the key is only consumed when it acts.
            if (event.keyCode == KeyCode.h || event.charactersIgnoringModifiers?.lowercased() == "h")
                && flags == [.command, .shift]
            {
                if onHideSelectedApp?() == true {
                    return nil
                }
                return event
            }

            if (event.keyCode == KeyCode.h || event.charactersIgnoringModifiers?.lowercased() == "h")
                && flags == [.command]
            {
                if !inCommandMode() {
                    onToggleHelp()
                }
                return nil
            }

            if (event.keyCode == KeyCode.p || event.charactersIgnoringModifiers?.lowercased() == "p")
                && flags == [.command]
            {
                if !inCommandMode() {
                    onTogglePick()
                }
                return nil
            }

            if (event.keyCode == KeyCode.p || event.charactersIgnoringModifiers?.lowercased() == "p")
                && flags == [.command, .shift]
            {
                if !inCommandMode() {
                    onClearPicked()
                }
                return nil
            }

            if (event.keyCode == KeyCode.returnKey || event.keyCode == KeyCode.keypadEnter) && flags == [.command, .shift] {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                    onSelectCommandByIndex(1)
                }
                return nil
            }

            // Shift+Enter opens every picked file/folder at once. Only when
            // there are picks; otherwise fall through so plain submit still
            // opens the selected result.
            if (event.keyCode == KeyCode.returnKey || event.keyCode == KeyCode.keypadEnter) && flags == [.shift] {
                if !inCommandMode() && hasPickedItems() {
                    onOpenAllPicked()
                    return nil
                }
                return event
            }

            // Cmd+D (keyCode 2) → trash the selection. Only in result mode; in
            // command mode it falls through so it keeps any text-editing meaning
            // in the command input.
            if (event.keyCode == KeyCode.d || event.charactersIgnoringModifiers?.lowercased() == "d")
                && flags == [.command]
                && !inCommandMode()
            {
                onRequestDelete?()
                return nil
            }

            // Cmd+O toggles the selected result's toggle Quick Action (Bluetooth,
            // etc.). Multi-choice controls will use Cmd+J/K in a later pass.
            // Match on the typed character only, not a hardware keyCode: 31 is
            // the physical ANSI-O position, which types another letter on
            // Dvorak/Colemak and would hijack that chord. Only swallow the
            // event when the selection actually has a toggle to act on.
            if event.charactersIgnoringModifiers?.lowercased() == "o"
                && flags == [.command]
                && !inCommandMode()
                && hasToggleQuickAction()
            {
                onToggleQuickAction?()
                return nil
            }

            if event.modifierFlags.contains(.command) && !event.modifierFlags.contains(.control)
                && !event.modifierFlags.contains(.option)
            {
                // macOS digit keyCodes are not contiguous: 1=18, 2=19, 3=20, 4=21, 5=23, 6=22, 7=26, 8=28, 9=25.
                let cmdNumberKey: Int?
                switch event.keyCode {
                case 18: cmdNumberKey = 1
                case 19: cmdNumberKey = 2
                case 20: cmdNumberKey = 3
                case 21: cmdNumberKey = 4
                case 23: cmdNumberKey = 5
                case 22: cmdNumberKey = 6
                case 26: cmdNumberKey = 7
                case 28: cmdNumberKey = 8
                case 25: cmdNumberKey = 9
                default: cmdNumberKey = nil
                }
                if let key = cmdNumberKey {
                    if inCommandMode() {
                        if key <= AppConstants.Launcher.commandCatalog.count {
                            Self.logger.debug("⌘+\(key, privacy: .public) -> command catalog")
                            DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                                onSelectCommandByIndex(key)
                            }
                            return nil
                        }
                        Self.logger.debug(
                            "⌘+\(key, privacy: .public) ignored (command mode maps 1-\(AppConstants.Launcher.commandCatalog.count, privacy: .public))")
                    } else {
                        Self.logger.debug("⌘+\(key, privacy: .public) -> running-apps switcher")
                        if onActivateRunningApp(key) {
                            return nil
                        }
                        Self.logger.debug(
                            "⌘+\(key, privacy: .public) running-apps activation declined, falling through"
                        )
                    }
                }
            }

            // Shift+Esc leaves AI mode straight to home (skips the list step).
            // Only consume it when it actually acts, so Shift+Esc keeps its
            // command-mode "hide" meaning elsewhere.
            if event.keyCode == KeyCode.escape,
                flags.contains(.shift),
                !flags.contains(.command),
                !flags.contains(.option),
                !flags.contains(.control),
                onEscapeHome?() == true
            {
                return nil
            }

            if event.modifierFlags.contains(.command)
                || event.modifierFlags.contains(.option)
                || event.modifierFlags.contains(.control)
            {
                Self.logKey("passthrough keyCode=\(event.keyCode) (modifier key combo)")
                return event
            }

            if event.keyCode == KeyCode.escape {
                // A pending launchpad Restart / Shut Down confirm swallows Escape
                // to dismiss the prompt rather than hiding the launcher.
                if let onLaunchpadEscape, onLaunchpadEscape() {
                    return nil
                }

                if onDismissHelpIfVisible() {
                    return nil
                }

                if killConfirmationActive() {
                    onCancelKill?()
                    return nil
                }

                if deleteConfirmationActive() {
                    onCancelDelete?()
                    return nil
                }

                if actionConfirmationActive() {
                    onCancelAction?()
                    return nil
                }

                if inCommandMode() {
                    if flags.contains(.shift) {
                        onHideLauncher()
                    } else {
                        onExitCommandMode()
                    }
                } else {
                    onHideLauncher()
                }
                return nil
            }

            if killConfirmationActive() {
                let char = event.charactersIgnoringModifiers?.lowercased()
                if char == "y" {
                    onConfirmKill?()
                    return nil
                }
                if char == "n" {
                    onCancelKill?()
                    return nil
                }
            }

            if deleteConfirmationActive() {
                // Enter confirms too - and must be swallowed so it doesn't fall
                // through to handleSubmit and *open* the file being deleted.
                if event.keyCode == KeyCode.returnKey || event.keyCode == KeyCode.keypadEnter {
                    onConfirmDelete?()
                    return nil
                }
                let char = event.charactersIgnoringModifiers?.lowercased()
                if char == "y" {
                    onConfirmDelete?()
                    return nil
                }
                if char == "n" {
                    onCancelDelete?()
                    return nil
                }
            }

            if event.keyCode == KeyCode.tab {
                if event.modifierFlags.contains(.shift) {
                    onPrevious()
                } else {
                    onNext()
                }
                return nil
            }

            // Shift+↑/↓ recalls prompt history in AI mode. The handler returns
            // false outside AI mode, so the event falls through to normal
            // selection-extension there.
            if event.keyCode == KeyCode.arrowUp, flags.contains(.shift) {
                if onRecallPrompt?(true) == true { return nil }
                return event
            }
            if event.keyCode == KeyCode.arrowDown, flags.contains(.shift) {
                if onRecallPrompt?(false) == true { return nil }
                return event
            }

            if event.keyCode == KeyCode.arrowUp {
                if let onArrowUp {
                    onArrowUp()
                } else {
                    onPrevious()
                }
                return nil
            }

            if event.keyCode == KeyCode.arrowDown {
                if let onArrowDown {
                    onArrowDown()
                } else {
                    onNext()
                }
                return nil
            }

            return event
        }
    }

    func stop() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
    }
}
