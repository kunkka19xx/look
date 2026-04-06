import AppKit
import Foundation

final class KeyboardSelectionMonitor {
    private var monitor: Any?
    private var isKillConfirmationActive: () -> Bool = { false }

    func start(
        onNext: @escaping () -> Void,
        onPrevious: @escaping () -> Void,
        onEnterCommandMode: @escaping () -> Void,
        onExitCommandMode: @escaping () -> Void,
        onHideLauncher: @escaping () -> Void,
        inCommandMode: @escaping () -> Bool,
        onBackToCommandList: @escaping () -> Void,
        onWebSearch: @escaping () -> Void,
        onSelectCommandByIndex: @escaping (Int) -> Void,
        onConfirmKill: (() -> Void)? = nil,
        onCancelKill: (() -> Void)? = nil,
        killConfirmationActive: @escaping () -> Bool = { false },
        onEnterClipboardMode: @escaping () -> Void = {},
        onExitClipboardMode: @escaping () -> Void = {},
        inClipboardMode: @escaping () -> Bool = { false }
    ) {
        guard monitor == nil else { return }
        self.isKillConfirmationActive = killConfirmationActive

        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

            if inClipboardMode() {
                if event.keyCode == 53 {
                    onExitClipboardMode()
                    return nil
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
                    onPrevious()
                    return nil
                }
                if event.keyCode == 125 {
                    onNext()
                    return nil
                }
                return event
            }

            if flags.contains(.command)
                && !flags.contains(.control)
                && !flags.contains(.option)
                && (event.keyCode == 44
                    || event.charactersIgnoringModifiers == "/"
                    || event.charactersIgnoringModifiers == "?")
            {
                onEnterCommandMode()
                return nil
            }

            if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command] {
                onWebSearch()
                return nil
            }

            if (event.keyCode == 36 || event.keyCode == 76) && flags == [.command, .shift] {
                onSelectCommandByIndex(1)
                return nil
            }

            if event.keyCode == 53 && event.modifierFlags.contains(.command) {
                onBackToCommandList()
                return nil
            }

            if event.modifierFlags.contains(.command) && !event.modifierFlags.contains(.control) && !event.modifierFlags.contains(.option) {
                let keyNumber = Int(event.keyCode)
                if keyNumber >= 18 && keyNumber <= 21 {
                    let index = keyNumber - 17
                    onSelectCommandByIndex(index)
                    return nil
                }
            }

            if event.modifierFlags.contains(.command)
                || event.modifierFlags.contains(.option)
                || event.modifierFlags.contains(.control)
            {
                return event
            }

            if event.keyCode == 53 {
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
                onPrevious()
                return nil
            }

            if event.keyCode == 125 {
                onNext()
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
