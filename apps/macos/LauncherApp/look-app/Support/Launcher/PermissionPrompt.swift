import AppKit

/// Runs a TCC request with the launcher pinned open.
///
/// The system dialog takes focus, which fires `didResignActive` and hides the
/// window. The app is then in the background, where macOS will not present the
/// next prompt, so a sequence of requests dies after the first one.
@MainActor
enum PermissionPrompt {
    private(set) static var isPresenting = false

    static func run(_ request: () async -> Void) async {
        isPresenting = true
        NSApplication.shared.activate(ignoringOtherApps: true)
        await request()
        isPresenting = false
    }
}
