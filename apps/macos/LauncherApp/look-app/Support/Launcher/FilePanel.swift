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

    static func runModal(_ panel: NSSavePanel) -> NSApplication.ModalResponse {
        isPresenting = true
        let response = panel.runModal()
        // The picker hands focus back to whatever was in front of it, which is
        // not us. Take it back, so the launcher is key and not merely visible.
        NSApplication.shared.activate(ignoringOtherApps: true)
        Task { @MainActor in
            try? await Task.sleep(for: settleWindow)
            isPresenting = false
        }
        return response
    }
}
