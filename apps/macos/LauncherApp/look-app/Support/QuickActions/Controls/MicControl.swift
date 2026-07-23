import Foundation

/// Mutes and reports the microphone. macOS has no true global mic mute, so this
/// zeroes the system input volume (gain) and restores the prior level on unmute.
/// Action id: `"mic"`. "On" means the mic is live (non-zero input volume). The
/// scripting bridge runs off the main thread via `AppleScriptRunner`.
struct MicControl: SystemControl {
    private static let restoreKey = "look.mic.preMuteInputVolume"
    /// Fallback level when unmuting with no remembered value (0-100).
    private static let defaultRestoreVolume = 75

    func state() async -> ActionState {
        guard let volume = await inputVolume() else { return .unavailable("No microphone") }
        return volume > 0 ? .on : .off
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard let current = await inputVolume() else { return .failed("No microphone") }
        let mute: Bool
        switch intent {
        case .toggle: mute = current > 0
        case .setOn(let on): mute = !on  // "on" = mic live = unmuted
        case .run: return .failed("Mic has no run action")
        }
        return mute ? await self.mute(current: current) : await unmute()
    }

    private func mute(current: Int) async -> ActionOutcome {
        // Remember the level so unmute can restore it (input volume 0 loses it).
        UserDefaults.standard.set(current, forKey: Self.restoreKey)
        guard await setInputVolume(0) else { return .failed("Could not mute mic") }
        return .ok(banner: "Mic muted")
    }

    private func unmute() async -> ActionOutcome {
        let saved = UserDefaults.standard.integer(forKey: Self.restoreKey)
        let restore = saved > 0 ? saved : Self.defaultRestoreVolume
        guard await setInputVolume(restore) else { return .failed("Could not unmute mic") }
        return .ok(banner: "Mic on")
    }

    /// Reads the system input volume (0-100), or nil when there is no input
    /// device. Needs no Automation consent (it isn't controlling another app).
    private func inputVolume() async -> Int? {
        await AppleScriptRunner.evaluateInt("input volume of (get volume settings)")
    }

    private func setInputVolume(_ volume: Int) async -> Bool {
        await AppleScriptRunner.runSucceeds("set volume input volume \(volume)")
    }
}
