import Foundation

/// Shuts the Mac down via System Events. Button-only: the launchpad gates it
/// behind an inline confirm before this runs. Action id: `"shutdown"`.
struct ShutdownControl: SystemControl {
    /// A power button has no readable on/off value.
    func state() async -> ActionState { .value("") }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard intent == .run else { return .failed("Shut Down has no toggle") }
        return await MainActor.run {
            AppleScriptRunner.run(
                "tell application \"System Events\" to shut down",
                successBanner: "Shutting down…",
                failureMessage: "Could not shut down"
            )
        }
    }
}
