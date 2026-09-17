import AppKit
import SwiftUI

/// The key capsule of a rebindable shortcut. Clicking it listens for the next
/// key press, core checks the combination, and a valid one lands in `bindings`
/// under the shortcut's config key; Save Config writes it like any other value.
struct ShortcutRecorderField: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @ObservedObject private var launcherHotkey = LauncherHotkeyController.shared

    let shortcut: ConfigurableShortcut
    @Binding var bindings: [String: String]

    @State private var isRecording = false
    @State private var error: String?
    @State private var monitor: Any?

    private enum Metrics {
        static let spacing: CGFloat = 6
        static let statusSpacing: CGFloat = 2
        static let horizontalPadding: CGFloat = 8
        static let verticalPadding: CGFloat = 3
        static let idleFillOpacity = 0.14
        static let recordingFillOpacity = 0.28
    }

    private enum Copy {
        static let listening = "Press a shortcut, Esc to cancel"
        static let reset = "Reset"
        static let unknownKey = "That key cannot be used"
    }

    private var fontSize: CGFloat { CGFloat(themeStore.settings.fontSize - 1) }

    private var pendingDisplay: String? {
        shortcut.pendingDisplay(in: bindings)
    }

    private var label: String {
        isRecording ? Copy.listening : (pendingDisplay ?? shortcut.currentDisplay)
    }

    private var isUnsaved: Bool {
        shortcut.hasUnsavedChange(in: bindings)
    }

    private var outlineColor: Color {
        isRecording || isUnsaved ? themeStore.accentColor() : .clear
    }

    private var canReset: Bool {
        guard let defaultSpec = shortcut.defaultSpec,
            let defaultDisplay = EngineBridge.shared.hotkeyCheck(defaultSpec)?.display
        else {
            return false
        }
        return (pendingDisplay ?? shortcut.currentDisplay) != defaultDisplay
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Metrics.statusSpacing) {
            HStack(spacing: Metrics.spacing) {
                Button(action: toggleRecording) {
                    Text(label)
                        .font(themeStore.uiFont(size: fontSize, weight: isRecording ? .semibold : .regular))
                        .padding(.horizontal, Metrics.horizontalPadding)
                        .padding(.vertical, Metrics.verticalPadding)
                        .background(
                            themeStore.liftColor(
                                opacity: isRecording ? Metrics.recordingFillOpacity : Metrics.idleFillOpacity),
                            in: Capsule()
                        )
                        .overlay(Capsule().strokeBorder(outlineColor))
                }
                .buttonStyle(.plain)
                .pointingHandCursor()

                if canReset && !isRecording {
                    Button(Copy.reset, action: reset)
                        .buttonStyle(.plain)
                        .font(themeStore.uiFont(size: fontSize, weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                        .pointingHandCursor()
                }
            }

            if let error {
                Text(error)
                    .font(themeStore.uiFont(size: fontSize - 1, weight: .regular))
                    .foregroundStyle(themeStore.dangerColor())
            }
        }
        .onDisappear(perform: stopRecording)
    }

    private func toggleRecording() {
        if isRecording {
            stopRecording()
        } else {
            startRecording()
        }
    }

    private func startRecording() {
        guard monitor == nil else { return }
        error = nil
        isRecording = true
        ShortcutCapture.isActive = true
        shortcut.suspend()
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            record(event)
            return nil
        }
    }

    private func stopRecording() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
        error = nil
        isRecording = false
        ShortcutCapture.isActive = false
        shortcut.resume()
    }

    private func record(_ event: NSEvent) {
        let modifiers = event.modifierFlags.intersection(CarbonHotkey.modifierMask)
        if event.keyCode == AppConstants.Launcher.KeyCode.escape, modifiers.isEmpty {
            stopRecording()
            return
        }
        guard let spec = CarbonHotkey.spec(for: event),
            let check = EngineBridge.shared.hotkeyCheck(spec)
        else {
            error = Copy.unknownKey
            return
        }
        if let rejection = check.error {
            error = rejection
            return
        }
        bindings[shortcut.configKey] = check.spec
        stopRecording()
    }

    private func reset() {
        guard let defaultSpec = shortcut.defaultSpec else { return }
        error = nil
        bindings[shortcut.configKey] = defaultSpec
    }
}
