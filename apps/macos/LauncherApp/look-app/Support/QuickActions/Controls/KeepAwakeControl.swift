import Foundation

/// Owns the "keep awake" power assertion. A struct adapter can't hold the
/// activity token across calls, so this shared object does; the token is held
/// for as long as keep-awake is on and released when it turns off.
@MainActor
final class KeepAwakeManager {
    static let shared = KeepAwakeManager()

    private var token: (any NSObjectProtocol)?

    var isActive: Bool { token != nil }

    func setActive(_ active: Bool) {
        if active {
            guard token == nil else { return }
            token = ProcessInfo.processInfo.beginActivity(
                options: [.idleSystemSleepDisabled, .idleDisplaySleepDisabled],
                reason: "Keep Awake"
            )
        } else if let token {
            ProcessInfo.processInfo.endActivity(token)
            self.token = nil
        }
    }
}

/// Toggles a keep-awake assertion so the display and system do not sleep while
/// on. Uses the public `ProcessInfo` activity API (no subprocess, no root).
/// Action id: `"keepawake"`. State is in-memory: it resets to off if Look
/// restarts. "On" means the assertion is held.
struct KeepAwakeControl: SystemControl {
    func state() async -> ActionState {
        await MainActor.run { KeepAwakeManager.shared.isActive ? .on : .off }
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        let target: Bool
        switch intent {
        case .toggle:
            target = await MainActor.run { !KeepAwakeManager.shared.isActive }
        case .setOn(let on):
            target = on
        case .run:
            return .failed("Keep Awake has no run action")
        }
        await MainActor.run { KeepAwakeManager.shared.setActive(target) }
        return .ok(banner: target ? "Keep Awake on" : "Keep Awake off")
    }
}
