import Foundation

/// Mutes and reports the microphone. macOS has no true global mic mute, so this
/// zeroes the system input volume (gain) and restores the prior level on unmute.
/// Action id: `"mic"`. "On" means the mic is live (non-zero input volume).
struct MicControl: SystemControl {
    private static let restoreKey = "look.mic.preMuteInputVolume"
    /// Fallback level when unmuting with no remembered value (0-100).
    private static let defaultRestoreVolume = 75

    func state() async -> ActionState {
        guard let volume = await MainActor.run(body: { Self.inputVolume() }) else {
            return .unavailable("No microphone")
        }
        return volume > 0 ? .on : .off
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard let current = await MainActor.run(body: { Self.inputVolume() }) else {
            return .failed("No microphone")
        }
        let mute: Bool
        switch intent {
        case .toggle: mute = current > 0
        case .setOn(let on): mute = !on  // "on" = mic live = unmuted
        case .run: return .failed("Mic has no run action")
        }
        return await MainActor.run { mute ? Self.mute(current: current) : Self.unmute() }
    }

    @MainActor
    private static func mute(current: Int) -> ActionOutcome {
        // Remember the level so unmute can restore it (input volume 0 loses it).
        UserDefaults.standard.set(current, forKey: restoreKey)
        guard setInputVolume(0) else { return .failed("Could not mute mic") }
        return .ok(banner: "Mic muted")
    }

    @MainActor
    private static func unmute() -> ActionOutcome {
        let saved = UserDefaults.standard.integer(forKey: restoreKey)
        let restore = saved > 0 ? saved : defaultRestoreVolume
        guard setInputVolume(restore) else { return .failed("Could not unmute mic") }
        return .ok(banner: "Mic on")
    }

    /// Reads the system input volume (0-100) via a scripting addition. This needs
    /// no Automation consent (it isn't controlling another app).
    @MainActor
    private static func inputVolume() -> Int? {
        var error: NSDictionary?
        let result = NSAppleScript(source: "input volume of (get volume settings)")?
            .executeAndReturnError(&error)
        guard error == nil, let result else { return nil }
        return Int(result.int32Value)
    }

    @MainActor
    private static func setInputVolume(_ volume: Int) -> Bool {
        var error: NSDictionary?
        NSAppleScript(source: "set volume input volume \(volume)")?.executeAndReturnError(&error)
        return error == nil
    }
}
