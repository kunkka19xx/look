import AppKit
import Foundation
import OSLog

// Wraps a non-Sendable value so it can be ferried across an
// actor-isolation boundary at compile time. Used here to pass NSEvent
// (and the resulting NSEvent?) into / out of MainActor.assumeIsolated.
// Safe in this file because NSEvent local-monitor handlers always fire
// on the main thread — no actual cross-thread access happens.
private struct UncheckedSendableBox<T>: @unchecked Sendable {
    let value: T
    init(_ value: T) { self.value = value }
}

@MainActor
final class KeyboardSelectionMonitor {
    private var monitor: Any?
    private var isKillConfirmationActive: @MainActor () -> Bool = { false }
    nonisolated private static let logger = Logger(subsystem: "noah-code.Look", category: "ui-key")
    nonisolated private static let debugKeyLoggingEnabled: Bool = {
        let env = ProcessInfo.processInfo.environment
        let raw = env["LOOK_UI_DEBUG_EVENTS"] ?? env["LOOK_DEV_HINT"] ?? ""
        return ["1", "true", "yes", "on"].contains(raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
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
        onToggleHelp: @escaping @MainActor () -> Void,
        onDismissHelpIfVisible: @escaping @MainActor () -> Bool,
        onSelectCommandByIndex: @escaping @MainActor (Int) -> Void,
        onConfirmKill: (@MainActor () -> Void)? = nil,
        onCancelKill: (@MainActor () -> Void)? = nil,
        killConfirmationActive: @escaping @MainActor () -> Bool = { false }
    ) {
        guard monitor == nil else { return }
        self.isKillConfirmationActive = killConfirmationActive

        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            // NSEvent local monitor handlers always fire on the main
            // thread; box + assumeIsolated lets us call MainActor-isolated
            // callbacks without an async hop while satisfying Swift 6's
            // actor-isolation check (NSEvent isn't Sendable).
            let inBox = UncheckedSendableBox(event)
            let outBox: UncheckedSendableBox<NSEvent?> = MainActor.assumeIsolated {
                let event = inBox.value
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
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command] {
                    onWebSearch()
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if (event.keyCode == 3 || event.charactersIgnoringModifiers?.lowercased() == "f")
                    && flags == [.command]
                {
                    onRevealInFinder()
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if (event.keyCode == 8 || event.charactersIgnoringModifiers?.lowercased() == "c")
                    && flags == [.command]
                {
                    if onCopySelection() {
                        return UncheckedSendableBox<NSEvent?>(nil)
                    }
                    return UncheckedSendableBox<NSEvent?>(event)
                }

                if (event.keyCode == 4 || event.charactersIgnoringModifiers?.lowercased() == "h")
                    && flags == [.command]
                {
                    if !inCommandMode() {
                        onToggleHelp()
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if (event.keyCode == 35 || event.charactersIgnoringModifiers?.lowercased() == "p")
                    && flags == [.command]
                {
                    if !inCommandMode() {
                        onTogglePick()
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if (event.keyCode == 35 || event.charactersIgnoringModifiers?.lowercased() == "p")
                    && flags == [.command, .shift]
                {
                    if !inCommandMode() {
                        onClearPicked()
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command, .shift] {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                        onSelectCommandByIndex(1)
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if event.modifierFlags.contains(.command) && !event.modifierFlags.contains(.control) && !event.modifierFlags.contains(.option) {
                    // macOS digit keyCodes are not contiguous: 1=18, 2=19, 3=20, 4=21, 5=23.
                    let mappedIndex: Int?
                    switch event.keyCode {
                    case 18: mappedIndex = 1
                    case 19: mappedIndex = 2
                    case 20: mappedIndex = 3
                    case 21: mappedIndex = 4
                    case 23: mappedIndex = 5
                    default: mappedIndex = nil
                    }
                    if let index = mappedIndex {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                            onSelectCommandByIndex(index)
                        }
                        return UncheckedSendableBox<NSEvent?>(nil)
                    }
                }

                if event.modifierFlags.contains(.command)
                    || event.modifierFlags.contains(.option)
                    || event.modifierFlags.contains(.control)
                {
                    Self.logKey("passthrough keyCode=\(event.keyCode) (modifier key combo)")
                    return UncheckedSendableBox<NSEvent?>(event)
                }

                if event.keyCode == 53 {
                    if onDismissHelpIfVisible() {
                        return UncheckedSendableBox<NSEvent?>(nil)
                    }

                    if killConfirmationActive() {
                        onCancelKill?()
                        return UncheckedSendableBox<NSEvent?>(nil)
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
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if killConfirmationActive() {
                    let char = event.charactersIgnoringModifiers?.lowercased()
                    if char == "y" {
                        onConfirmKill?()
                        return UncheckedSendableBox<NSEvent?>(nil)
                    }
                    if char == "n" {
                        onCancelKill?()
                        return UncheckedSendableBox<NSEvent?>(nil)
                    }
                }

                if event.keyCode == 48 {
                    if event.modifierFlags.contains(.shift) {
                        onPrevious()
                    } else {
                        onNext()
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if event.keyCode == 126 {
                    if let onArrowUp {
                        onArrowUp()
                    } else {
                        onPrevious()
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                if event.keyCode == 125 {
                    if let onArrowDown {
                        onArrowDown()
                    } else {
                        onNext()
                    }
                    return UncheckedSendableBox<NSEvent?>(nil)
                }

                return UncheckedSendableBox<NSEvent?>(event)
            }
            return outBox.value
        }
    }

    func stop() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
    }
}
