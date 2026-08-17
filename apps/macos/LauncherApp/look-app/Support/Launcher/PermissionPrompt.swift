import AppKit

/// Runs a TCC request with the launcher pinned open.
///
/// The system dialog takes focus, which fires `didResignActive` and hides the
/// window. The app is then in the background, where macOS will not present the
/// next prompt, so a sequence of requests dies after the first one.
@MainActor
enum PermissionPrompt {
    /// Counted, not a flag: `await` inside `run` is a suspension point, so a
    /// second request can start before the first returns, and the first to
    /// finish would otherwise unpin the window while a dialog is still up.
    private static var active = 0

    static var isPresenting: Bool { active > 0 }

    static func run(_ request: () async -> Void) async {
        active += 1
        defer { active -= 1 }
        NSApplication.shared.activate(ignoringOtherApps: true)
        await request()
    }
}
