import Foundation

/// When a meeting starts, in the words a row has space for.
///
/// Its own type rather than a helper on the launcher view: the picker is built
/// in `ActionController`, and a controller reaching into a `View` for wording
/// would be the wrong way round.
nonisolated enum MeetingTiming {
    private enum Copy {
        static let inProgress = "in progress"
        static let startingNow = "starting now"
        /// Past an hour a countdown in minutes stops being readable ("in 1440
        /// min") and the start time itself is the useful thing to say.
        static let minutesPerHour = 60
    }

    /// A meeting under way says so rather than counting negative minutes, and
    /// one that is not today says WHEN rather than counting to 1440.
    static func phrase(_ meeting: JoinableMeeting) -> String {
        if meeting.inProgress { return Copy.inProgress }
        let minutes = meeting.minutesUntilStart
        guard minutes > 0 else { return Copy.startingNow }
        if minutes < Copy.minutesPerHour { return "in \(minutes) min" }

        let start = meeting.startDate
        let clock = clockTime.string(from: start)
        let calendar = Calendar.current
        if calendar.isDateInToday(start) { return "at \(clock)" }
        if calendar.isDateInTomorrow(start) { return "tomorrow \(clock)" }
        return "\(weekday.string(from: start)) \(clock)"
    }

    /// The start time in the user's own clock format.
    static let clockTime: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    /// "Thu" - enough to place a meeting inside the two-day window the service
    /// looks over.
    private static let weekday: DateFormatter = {
        let formatter = DateFormatter()
        formatter.setLocalizedDateFormatFromTemplate("EEE")
        return formatter
    }()
}
