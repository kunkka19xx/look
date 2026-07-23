import AppKit
import Foundation
import OSLog

/// Starts the screensaver. Button-only, no readable state. Action id:
/// `"screensaver"`.
struct ScreensaverControl: SystemControl {
    private static let log = Logger(subsystem: "noah-code.Look", category: "actions.screensaver")
    private static let enginePath = "/System/Library/CoreServices/ScreenSaverEngine.app"

    /// A button has no on/off value to display.
    func state() async -> ActionState { .value("") }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard intent == .run else { return .failed("Screensaver has no toggle") }
        let url = URL(fileURLWithPath: Self.enginePath)
        do {
            try await NSWorkspace.shared.openApplication(at: url, configuration: .init())
            return .ok(banner: "Screensaver")
        } catch {
            Self.log.error("screensaver launch failed: \(error.localizedDescription, privacy: .public)")
            return .failed("Could not start screensaver")
        }
    }
}
