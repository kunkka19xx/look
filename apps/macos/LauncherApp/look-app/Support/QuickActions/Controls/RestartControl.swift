import Foundation

/// Restarts the Mac via System Events. Button-only: the launchpad gates it behind
/// an inline confirm before this runs. Action id: `"restart"`.
struct RestartControl: SystemControl {
    /// A power button has no readable on/off value.
    func state() async -> ActionState { .value("") }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard intent == .run else { return .failed("Restart has no toggle") }
        return await AppleScriptRunner.run(
            "tell application \"System Events\" to restart",
            successBanner: "Restarting…",
            failureMessage: "Could not restart"
        )
    }
}
