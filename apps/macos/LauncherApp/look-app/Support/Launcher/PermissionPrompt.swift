import AppKit

/// Runs a TCC request so its dialog is visible and a sequence of them can
/// finish.
///
/// Two things get in the way. The system draws the dialog at normal window
/// level, and the launcher floats above that, so it opens behind the launcher.
/// And hiding on `didResignActive` backgrounds the app, where macOS will not
/// present the next prompt at all.
@MainActor
enum PermissionPrompt {
    /// The launcher's own minimum frame. Smaller windows are the status item
    /// and popover anchors, whose levels must stay as they are.
    private static let launcherMinimumSide: CGFloat = 400

    /// Counted, not a flag: `await` inside `run` is a suspension point, so a
    /// second request can start before the first returns, and the first to
    /// finish would otherwise restore the level while a dialog is still up.
    private static var active = 0
    private static var restoreLevels: [(window: NSWindow, level: NSWindow.Level)] = []

    static var isPresenting: Bool { active > 0 }

    static func run(_ request: () async -> Void) async {
        beginPresenting()
        defer { endPresenting() }
        await request()
    }

    private static func beginPresenting() {
        active += 1
        guard active == 1 else { return }
        NSApplication.shared.activate(ignoringOtherApps: true)
        restoreLevels = launcherWindows().map { ($0, $0.level) }
        for entry in restoreLevels { entry.window.level = .normal }
    }

    private static func endPresenting() {
        active -= 1
        guard active == 0 else { return }
        for entry in restoreLevels { entry.window.level = entry.level }
        restoreLevels = []
    }

    private static func launcherWindows() -> [NSWindow] {
        NSApplication.shared.windows.filter {
            $0.isVisible
                && $0.frame.width >= launcherMinimumSide
                && $0.frame.height >= launcherMinimumSide
        }
    }
}
