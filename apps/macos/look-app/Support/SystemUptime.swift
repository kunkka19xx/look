import Foundation

/// Human-readable system uptime, e.g. "2d 3h 14m" (the day part is dropped under
/// 24h). Shared by the /sys command and the launchpad Battery/Uptime tile so both
/// format it identically.
nonisolated enum SystemUptime {
    private enum Const {
        static let secondsPerDay = 86400
        static let secondsPerHour = 3600
        static let secondsPerMinute = 60
    }

    static func formattedShort() -> String {
        let uptime = Int(ProcessInfo.processInfo.systemUptime)
        let days = uptime / Const.secondsPerDay
        let hours = (uptime % Const.secondsPerDay) / Const.secondsPerHour
        let minutes = (uptime % Const.secondsPerHour) / Const.secondsPerMinute

        var result = ""
        if days > 0 {
            result += "\(days)d "
        }
        result += "\(hours)h \(minutes)m"
        return result
    }
}
