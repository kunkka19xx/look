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

    /// How long to wait for the controller to apply a power change, and how often
    /// to re-check while waiting. The controller applies asynchronously (~100ms).
    private static let settleTimeout: TimeInterval = 1.5
    private static let pollInterval: UInt64 = 80_000_000  // 80ms in nanoseconds

    private func isPoweredOn() -> Bool {
        IOBluetoothPreferenceGetControllerPowerState() == 1
    }

    func state() async -> ActionState {
        // The read is cheap and synchronous; no need to hop threads.
        isPoweredOn() ? .on : .off
    }

    /// Resolves "status" to the paired-device list (connected first), so the
    /// panel can connect/disconnect each one. Falls back to a plain line when
    /// Bluetooth is off or nothing is paired.
    func info(keys: [String]) async -> [String: InfoValue] {
        guard keys.contains("status") else { return [:] }
        guard isPoweredOn() else { return ["status": .text("Off")] }
        // IOBluetoothDevice APIs are @MainActor in the SDK; hop there. The read
        // is cheap and returns only Sendable list items.
        let items = await MainActor.run { Self.pairedDeviceItems() }
        if items.isEmpty { return ["status": .text("On, no paired devices")] }
        return ["status": .list(items)]
    }

    /// Connects/disconnects a paired device. `itemId` is its Bluetooth address.
    func applyItem(_ itemId: String, intent: ActionIntent) async -> ActionOutcome {
        guard intent == .toggle else {
            return .failed("Devices can only be connected or disconnected")
        }
        // The whole lookup + connect stays on the main actor (IOBluetoothDevice
        // is @MainActor); only the Sendable outcome crosses back.
        return await MainActor.run {
            guard let device = Self.pairedDevices().first(where: { $0.addressString == itemId }) else {
                return .failed("Device is no longer available")
            }
            let name = device.name ?? itemId
            let connect = !device.isConnected()
            let result: IOReturn = connect ? device.openConnection() : device.closeConnection()
            guard result == kIOReturnSuccess else {
                return .failed(connect ? "Failed to connect to \(name)" : "Failed to disconnect from \(name)")
            }
            return .ok(banner: connect ? "Connected to \(name)" : "Disconnected from \(name)")
        }
    }

    @MainActor
    private static func pairedDevices() -> [IOBluetoothDevice] {
        (IOBluetoothDevice.pairedDevices() as? [IOBluetoothDevice]) ?? []
    }

    /// Paired devices as list items, connected first then alphabetical.
    @MainActor
    private static func pairedDeviceItems() -> [QuickActionListItem] {
        pairedDevices()
            .compactMap { device -> QuickActionListItem? in
                guard let address = device.addressString else { return nil }
                return QuickActionListItem(id: address, label: device.name ?? address, on: device.isConnected())
            }
            .sorted { lhs, rhs in
                let lhsOn = lhs.on ?? false
                let rhsOn = rhs.on ?? false
                if lhsOn != rhsOn { return lhsOn }
                return lhs.label.localizedCaseInsensitiveCompare(rhs.label) == .orderedAscending
            }
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
        // The controller applies the change asynchronously (~100ms), so wait
        // until the power state actually reflects the target before returning.
        // Otherwise an immediate `state()` read would still see the old value
        // and the panel would trail a press behind.
        let settled = await waitForPowerState(target)
        Self.log.debug("bluetooth apply -> target=\(target, privacy: .public) settled=\(settled, privacy: .public)")
        guard settled else {
            return .failed("Could not turn Bluetooth \(target ? "on" : "off")")
        }
        return .ok(banner: "Bluetooth \(target ? "on" : "off")")
    }

    /// Polls the power state until it reaches `target` or the settle timeout.
    private func waitForPowerState(_ target: Bool) async -> Bool {
        let deadline = Date().addingTimeInterval(Self.settleTimeout)
        while Date() < deadline {
            if isPoweredOn() == target { return true }
            try? await Task.sleep(nanoseconds: Self.pollInterval)
        }
        return isPoweredOn() == target
    }
}
