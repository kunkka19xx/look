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
}
