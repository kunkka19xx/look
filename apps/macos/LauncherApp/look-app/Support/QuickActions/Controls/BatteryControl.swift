import Foundation
import IOKit.ps

/// Reports the current battery charge. Read-only: there is no action to run.
/// Action id: `"battery"`.
struct BatteryControl: SystemControl {
    func state() async -> ActionState {
        guard let percent = Self.batteryPercent() else {
            return .unavailable("No battery")
        }
        return .value("\(percent)%")
    }

    /// Resolves "charging" to whether the battery is actively drawing power,
    /// per `kIOPSIsChargingKey`. Mirrors the linows adapters' `info()`.
    func info(keys: [String]) async -> [String: InfoValue] {
        guard keys.contains("charging") else { return [:] }
        return ["charging": .text(Self.isCharging() ? "charging" : "idle")]
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        .failed("Battery is read-only")
    }

    /// Current charge as a whole percent, or nil on a machine with no battery
    /// (e.g. a desktop Mac), by reading the internal power source via IOKit.
    private static func batteryPercent() -> Int? {
        guard let snapshot = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let sources = IOPSCopyPowerSourcesList(snapshot)?.takeRetainedValue() as? [CFTypeRef] else {
            return nil
        }
        for source in sources {
            guard let description = IOPSGetPowerSourceDescription(snapshot, source)?
                    .takeUnretainedValue() as? [String: Any],
                  let current = description[kIOPSCurrentCapacityKey as String] as? Int,
                  let max = description[kIOPSMaxCapacityKey as String] as? Int,
                  max > 0 else {
                continue
            }
            return Int((Double(current) / Double(max) * 100).rounded())
        }
        return nil
    }

    /// Whether the internal power source is actively charging via IOKit.
    private static func isCharging() -> Bool {
        guard let snapshot = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let sources = IOPSCopyPowerSourcesList(snapshot)?.takeRetainedValue() as? [CFTypeRef] else {
            return false
        }
        for source in sources {
            guard let description = IOPSGetPowerSourceDescription(snapshot, source)?
                    .takeUnretainedValue() as? [String: Any] else {
                continue
            }
            if let charging = description[kIOPSIsChargingKey as String] as? Bool {
                return charging
            }
        }
        return false
    }
}
