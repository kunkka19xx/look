import AppKit
import Foundation
import OSLog

final class KeyboardSelectionMonitor {
    private var monitor: Any?
    private var isKillConfirmationActive: () -> Bool = { false }
    private static let logger = Logger(subsystem: "noah-code.Look", category: "ui-key")
    private static let debugKeyLoggingEnabled: Bool = {
        let env = ProcessInfo.processInfo.environment
        let raw = env["LOOK_UI_DEBUG_EVENTS"] ?? env["LOOK_DEV_HINT"] ?? ""
        return ["1", "true", "yes", "on"].contains(raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    }()

    private static func logKey(_ message: String) {
        guard Self.debugKeyLoggingEnabled else { return }
        Self.logger.notice("\(message, privacy: .public)")
    }

    func start(
        onNext: @escaping () -> Void,
        onPrevious: @escaping () -> Void,
        onArrowDown: (() -> Void)? = nil,
        onArrowUp: (() -> Void)? = nil,
        onEnterCommandMode: @escaping () -> Void,
        onExitCommandMode: @escaping () -> Void,
        onHideLauncher: @escaping () -> Void,
        inCommandMode: @escaping () -> Bool,
        onWebSearch: @escaping () -> Void,
        onRevealInFinder: @escaping () -> Void,
        onCopySelection: @escaping () -> Bool,
        onToggleHelp: @escaping () -> Void,
        onDismissHelpIfVisible: @escaping () -> Bool,
        onSelectCommandByIndex: @escaping (Int) -> Void,
        onConfirmKill: (() -> Void)? = nil,
        onCancelKill: (() -> Void)? = nil,
        killConfirmationActive: @escaping () -> Bool = { false }
    ) {
        guard monitor == nil else { return }
        self.isKillConfirmationActive = killConfirmationActive

        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            Self.logKey("down keyCode=\(event.keyCode) chars=\(event.charactersIgnoringModifiers ?? "") flagsRaw=\(flags.rawValue) inCommand=\(inCommandMode())")

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

            if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command, .shift] {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                    onSelectCommandByIndex(1)
                }
                return nil
            }

            if event.modifierFlags.contains(.command) && !event.modifierFlags.contains(.control) && !event.modifierFlags.contains(.option) {
                let keyNumber = Int(event.keyCode)
                if keyNumber >= 18 && keyNumber <= 21 {
                    let index = keyNumber - 17
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                        onSelectCommandByIndex(index)
                    }
                    return nil
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
