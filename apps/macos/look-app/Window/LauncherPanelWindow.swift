import AppKit

/// Borderless, because macOS clips a *titled* window to its own corner radius
/// whatever `layer.cornerRadius` says, so the panel could never go square.
/// Borderless refuses key status by default, hence the overrides.
final class LauncherPanelWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}
