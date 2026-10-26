import Foundation

/// A control a declaration supplied, rather than one this app implements.
///
/// The native adapters in `ActionAdapterRegistry` each talk to a system API.
/// This one talks to whichever connector produced the row, so the same panel
/// renders both and nothing above here knows the difference.
struct ConnectorControl: SystemControl {
    let actionID: String

    /// Only ids the core marked as supplied. A native id never reaches here,
    /// which is the rule that stops a declaration borrowing the machine's
    /// Bluetooth.
    static func forAction(_ actionID: String) -> ConnectorControl? {
        actionID.hasPrefix(prefix) ? ConnectorControl(actionID: actionID) : nil
    }

    /// Mirrors `look_engine::qactions`.
    private static let prefix = "declared:"

    func state() async -> ActionState {
        read(press: false)
    }

    /// Reads back what the connector reached, never what we asked for: a
    /// connector may refuse, and a toggle that lies is worse than a slow one.
    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard case .unavailable(let reason) = read(press: true) else { return .ok(banner: nil) }
        return .failed(reason)
    }

    func info(keys: [String]) async -> [String: InfoValue] { [:] }

    func applyItem(_ itemId: String, intent: ActionIntent) async -> ActionOutcome {
        .failed("That control has no items")
    }

    private func read(press: Bool) -> ActionState {
        guard let answer = EngineBridge.shared.control(actionID: actionID, press: press) else {
            return .unavailable("The core did not answer")
        }
        if let error = answer.error {
            return .unavailable(error)
        }
        return answer.on ? .on : .off
    }
}
