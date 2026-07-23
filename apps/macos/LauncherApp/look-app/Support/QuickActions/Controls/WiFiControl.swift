import CoreWLAN
import Foundation
import OSLog

/// Toggles and reports Wi-Fi power via CoreWLAN. Action id: `"wifi"`.
struct WiFiControl: SystemControl {
    private static let log = Logger(subsystem: "noah-code.Look", category: "actions.wifi")

    /// The default Wi-Fi interface, or nil on a machine without Wi-Fi hardware.
    private var interface: CWInterface? {
        CWWiFiClient.shared().interface()
    }

    func state() async -> ActionState {
        guard let interface else { return .unavailable("No Wi-Fi hardware") }
        return interface.powerOn() ? .on : .off
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        guard let interface else { return .failed("No Wi-Fi hardware") }

        let target: Bool
        switch intent {
        case .toggle:
            target = !interface.powerOn()
        case .setOn(let on):
            target = on
        case .run:
            return .failed("Wi-Fi has no run action")
        }

        do {
            try interface.setPower(target)
            return .ok(banner: "Wi-Fi \(target ? "on" : "off")")
        } catch {
            Self.log.error("wifi setPower failed: \(error.localizedDescription, privacy: .public)")
            return .failed("Could not turn Wi-Fi \(target ? "on" : "off")")
        }
    }
}
