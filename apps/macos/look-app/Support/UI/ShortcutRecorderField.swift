import AppKit
import SwiftUI

/// The key capsule of a rebindable shortcut. Click, press a combination, and a
/// valid one waits in `bindings` until Save Config writes it.
struct ShortcutRecorderField: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @ObservedObject private var launcherHotkey = LauncherHotkeyController.shared
    /// Redraws the capsule when a rebind takes effect.
    @ObservedObject private var store = ShortcutBindings.shared

    let shortcut: ConfigurableShortcut
    @Binding var bindings: [String: String]

    @State private var error: String?
    @State private var monitor: Any?

    private static let listening = "Press a shortcut, Esc to cancel"
    private static let listeningForModifier = "Press Cmd, Ctrl or Option, Esc to cancel"
    private static let digitRowModifiers: NSEvent.ModifierFlags = [.command, .control, .option]
    private static let unknownKey = "That key cannot be used"
    private static let deafRecorder = "Could not listen for keys"
    private static let fillOpacity = 0.14
    private static let restingOutlineOpacity = 0.4
    private static let spacing: CGFloat = 6

    private var isRecording: Bool { monitor != nil }
    private var fontSize: CGFloat { CGFloat(themeStore.settings.fontSize - 1) }
    private var shown: String {
        shortcut.pendingDisplay(in: bindings) ?? shortcut.display
    }

    private var defaultSpec: String? {
        guard let spec = shortcut.resetSpec, shortcut.pendingDisplay(in: [shortcut.configKey: spec]) != shown else {
            return nil
        }
        return spec
    }

    /// A resting recorder still carries a border: it is what tells the reader
    /// this capsule is an input and the dimmed ones next to it are not.
    private var outline: Color {
        isRecording || shortcut.hasUnsavedChange(in: bindings)
            ? themeStore.accentColor()
            : themeStore.liftColor(opacity: Self.restingOutlineOpacity)
    }

    private var prompt: String {
        shortcut.isDigitRow ? Self.listeningForModifier : Self.listening
    }

    private var capsule: some View {
        Text(isRecording ? prompt : shown)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(themeStore.liftColor(opacity: Self.fillOpacity), in: Capsule())
            .overlay(Capsule().strokeBorder(outline))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: Self.spacing) {
                Button {
                    if isRecording { stopRecording() } else { startRecording() }
                } label: {
                    capsule
                }
                .buttonStyle(.plain)
                .pointingHandCursor()

                if let defaultSpec, !isRecording {
                    Button("Reset") { bindings[shortcut.configKey] = defaultSpec }
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.secondaryTextColor())
                        .pointingHandCursor()
                }
            }
            if let error {
                Text(error).foregroundStyle(themeStore.dangerColor())
            }
        }
        .font(themeStore.uiFont(size: fontSize, weight: .regular))
        .onDisappear(perform: stopRecording)
    }

    private func startRecording() {
        shortcut.suspend()
        let installed = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { event in
            record(event)
            return nil
        }
        guard let installed else {
            // stopRecording() will not run, so hand the shortcut back here.
            shortcut.reload()
            error = Self.deafRecorder
            return
        }
        monitor = installed
        ShortcutCapture.isActive = true
    }

    private func stopRecording() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
        error = nil
        ShortcutCapture.isActive = false
        shortcut.reload()
    }

    private func record(_ event: NSEvent) {
        let bare = event.modifierFlags.intersection(CarbonHotkey.modifierMask).isEmpty
        if event.keyCode == AppConstants.Launcher.KeyCode.escape, bare {
            stopRecording()
            return
        }
        // A digit row is set by its modifier alone.
        let modifierAlone = event.type == .flagsChanged
        if modifierAlone, !shortcut.isDigitRow || event.modifierFlags.isDisjoint(with: Self.digitRowModifiers) {
            return
        }
        let key = modifierAlone ? LocalShortcut.digitRowKey : nil
        guard let check = CarbonHotkey.spec(for: event, key: key).flatMap(shortcut.check) else {
            error = Self.unknownKey
            return
        }
        if let rejection = check.error ?? shortcut.unusableReason(check, in: bindings) {
            error = rejection
            return
        }
        bindings[shortcut.configKey] = check.spec
        stopRecording()
    }
}
