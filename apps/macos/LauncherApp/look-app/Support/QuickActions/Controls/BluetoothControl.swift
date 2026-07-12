import Foundation
import IOBluetooth
import OSLog

// ============================================================================
// REFERENCE ADAPTER - copy this file to add a new system control.
//
// A control conforms to `SystemControl` and keeps ALL of its OS-specific code
// (private APIs, AppleScript, CLI calls) inside itself. To add your own:
//
//   1. Copy this file and rename the type (e.g. `WiFiControl`).
//   2. Implement `state()` - read the current state, or `.unavailable(reason)`.
//   3. Implement `apply(_:)` - perform the change, return an `ActionOutcome`.
//   4. Register it in `ActionAdapterRegistry` under your action id.
//   5. Declare the matching descriptor in the shared `core/qactions` catalog.
//
// Nothing else (panel, keyboard, rendering) changes. That is the whole point.
// ============================================================================

// macOS has no public API to toggle system Bluetooth power. These private
// IOBluetooth C symbols do it; they are not in the public headers but have been
// stable for years and are what `blueutil` uses. They resolve at link time via
// `import IOBluetooth` (the framework autolinks). This is exactly the kind of
// OS-specific detail the adapter exists to contain.
@_silgen_name("IOBluetoothPreferenceGetControllerPowerState")
private func IOBluetoothPreferenceGetControllerPowerState() -> Int32

@_silgen_name("IOBluetoothPreferenceSetControllerPowerState")
private func IOBluetoothPreferenceSetControllerPowerState(_ state: Int32)

/// Toggles and reports macOS system Bluetooth power. Action id: `"bluetooth"`.
struct BluetoothControl: SystemControl {
    private static let log = Logger(subsystem: "noah-code.Look", category: "actions.bluetooth")

    private func isPoweredOn() -> Bool {
        IOBluetoothPreferenceGetControllerPowerState() == 1
    }

    func state() async -> ActionState {
        // The read is cheap and synchronous; no need to hop threads.
        isPoweredOn() ? .on : .off
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        let target: Bool
        switch intent {
        case .toggle:
            target = !isPoweredOn()
        case .setOn(let on):
            target = on
        case .run:
            return .failed("Bluetooth has no run action")
        }

        IOBluetoothPreferenceSetControllerPowerState(target ? 1 : 0)
        // The controller applies the change asynchronously, so we report the
        // intended result optimistically; the panel re-reads `state()` shortly
        // after and will reflect reality if the change did not take.
        Self.log.debug("bluetooth apply -> target=\(target, privacy: .public)")
        return .ok(banner: "Bluetooth \(target ? "on" : "off")")
    }
}
