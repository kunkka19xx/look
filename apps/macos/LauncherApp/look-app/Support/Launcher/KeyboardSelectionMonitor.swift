import AppKit
import Foundation
import OSLog

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
        onConfirmKill: (@MainActor () -> Void)? = nil,
        onCancelKill: (@MainActor () -> Void)? = nil,
        killConfirmationActive: @escaping @MainActor () -> Bool = { false },
        onRequestDelete: (@MainActor () -> Void)? = nil,
        onConfirmDelete: (@MainActor () -> Void)? = nil,
        onCancelDelete: (@MainActor () -> Void)? = nil,
        deleteConfirmationActive: @escaping @MainActor () -> Bool = { false }
    ) {
        guard monitor == nil else { return }
        self.isKillConfirmationActive = killConfirmationActive

        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            Self.logKey(
                "down keyCode=\(event.keyCode) chars=\(event.charactersIgnoringModifiers ?? "") flagsRaw=\(flags.rawValue) inCommand=\(inCommandMode())"
            )

            if flags.contains(.command)
                && !flags.contains(.control)
                && !flags.contains(.option)
                && (event.keyCode == 44
                    || event.charactersIgnoringModifiers == "/"
                    || event.charactersIgnoringModifiers == "?")
            {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                    onEnterCommandMode()
                }
                return nil
            }

            if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command] {
                onWebSearch()
                return nil
            }

            if (event.keyCode == 3 || event.charactersIgnoringModifiers?.lowercased() == "f")
                && flags == [.command]
            {
                onRevealInFinder()
                return nil
            }

            if (event.keyCode == 8 || event.charactersIgnoringModifiers?.lowercased() == "c")
                && flags == [.command]
            {
                if onCopySelection() {
                    return nil
                }
                return event
            }

            if (event.keyCode == 4 || event.charactersIgnoringModifiers?.lowercased() == "h")
                && flags == [.command]
            {
                if !inCommandMode() {
                    onToggleHelp()
                }
                return nil
            }

            if (event.keyCode == 35 || event.charactersIgnoringModifiers?.lowercased() == "p")
                && flags == [.command]
            {
                if !inCommandMode() {
                    onTogglePick()
                }
                return nil
            }

            if (event.keyCode == 35 || event.charactersIgnoringModifiers?.lowercased() == "p")
                && flags == [.command, .shift]
            {
                if !inCommandMode() {
                    onClearPicked()
                }
                return nil
            }

            if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command, .shift] {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                    onSelectCommandByIndex(1)
                }
                return nil
            }

            // Shift+Enter opens every picked file/folder at once. Only when
            // there are picks; otherwise fall through so plain submit still
            // opens the selected result.
            if (event.keyCode == 36 || event.keyCode == 76) && flags == [.shift] {
                if !inCommandMode() && hasPickedItems() {
                    onOpenAllPicked()
                    return nil
                }
                return event
            }

            // Cmd+D (keyCode 2) → trash the selection. Only in result mode; in
            // command mode it falls through so it keeps any text-editing meaning
            // in the command input.
            if (event.keyCode == 2 || event.charactersIgnoringModifiers?.lowercased() == "d")
                && flags == [.command]
                && !inCommandMode()
            {
                onRequestDelete?()
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

            if event.modifierFlags.contains(.command)
                || event.modifierFlags.contains(.option)
                || event.modifierFlags.contains(.control)
            {
                Self.logKey("passthrough keyCode=\(event.keyCode) (modifier key combo)")
                return event
            }

            if event.keyCode == 53 {
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
                // Enter confirms too — and must be swallowed so it doesn't fall
                // through to handleSubmit and *open* the file being deleted.
                if event.keyCode == 36 || event.keyCode == 76 {
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

            if event.keyCode == 48 {
                if event.modifierFlags.contains(.shift) {
                    onPrevious()
                } else {
                    onNext()
                }
                return nil
            }

            if event.keyCode == 126 {
                if let onArrowUp {
                    onArrowUp()
                } else {
                    onPrevious()
                }
                return nil
            }

            if event.keyCode == 125 {
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
