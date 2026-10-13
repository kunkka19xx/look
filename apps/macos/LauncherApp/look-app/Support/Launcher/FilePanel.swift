import AppKit

/// Runs an open/save panel without the launcher sliding out from under it.
///
/// The panel is drawn by another process, so the app resigns active for as
/// long as it is up, and `didResignActive` is what hides the launcher. The
/// hide is right when you click away to another app, and wrong when the thing
/// taking focus is our own file picker: you pick a picture, and the settings
/// screen you were going to tune the blur on has gone.
@MainActor
enum FilePanel {
    /// How long the suppression outlives the modal. The resign the panel
    /// caused can arrive a runloop turn or two after `runModal` returns, which
    /// would hide the launcher once the picker is already closed.
    private static let settleWindow: Duration = .milliseconds(400)

    private(set) static var isPresenting = false
    /// Held so a second picker cancels the first one's countdown: otherwise
    /// that one wakes while this panel is still up and clears the flag.
    private static var settle: Task<Void, Never>?

    static func runModal(_ panel: NSSavePanel) -> NSApplication.ModalResponse {
        settle?.cancel()
        isPresenting = true
        let response = panel.runModal()
        // The picker hands focus back to whatever was in front of it, which is
        // not us. Take it back, so the launcher is key and not merely visible.
        NSApplication.shared.activate(ignoringOtherApps: true)
        settle = Task { @MainActor in
            // A cancelled sleep returns rather than throws here, so the flag
            // needs its own guard.
            try? await Task.sleep(for: settleWindow)
            guard !Task.isCancelled else { return }
            isPresenting = false
        }
        return response
    }
}
