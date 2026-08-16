import Foundation

/// The pinned "Join <meeting>" row, plus the timing wording shared with the
/// AI-mode join picker. The grammar ("is this a join request?"),
/// the link hiding in the invite, and the choice of WHICH meeting all live in
/// the shared `core/ai` crate via `EngineBridge` and `MeetingService`, so this
/// file is presentation and placement only. Mirrors `LauncherView+Calc.swift`.
extension LauncherView {
    private enum Copy {
        static let inProgress = "in progress"
        static let startingNow = "starting now"
        static let enterHint = "Enter to join"
        /// Past this, a countdown in minutes stops being readable ("in 1440
        /// min") and the start time itself is the useful thing to say.
        static let minutesPerHour = 60
    }

    /// Join the highlighted meeting and get out of the way. The meeting app or
    /// the browser is taking the screen, so a launcher left open on a "Joining
    /// ..." bar is a dead end the user has to Esc out of. A failure keeps the
    /// panel up, since that is the case with something to read.
    func joinHighlightedMeeting() {
        guard actionController.joinSelectedMeeting() else { return }
        clearQuerySilently()
        hideLauncherWindow(restorePreviousApp: false)
    }

    /// A synthesized row for `join`-style queries, or nil. The calendar read
    /// behind this is cached (see `MeetingService`), so it is safe to evaluate
    /// on every keystroke like the other pinned rows.
    var meetingResult: LauncherResult? {
        guard allowsSuggestionRows, let request = bridge.joinQuery(query) else { return nil }
        // A named request that matches nothing shows no row at all, which is
        // what keeps "join two pdfs" an ordinary file search.
        guard let meeting = MeetingService.shared.nextMeeting(name: request.name ?? "") else {
            return nil
        }

        let timing = Self.meetingTiming(meeting)
        var result = LauncherResult(
            id: AppConstants.Launcher.Meeting.resultID(url: meeting.url),
            kind: .app,
            title: "Join \(meeting.title)",
            subtitle: "\(meeting.providerLabel)  •  \(timing)  •  \(Copy.enterHint)",
            // No path: the row opens a URL, and there is no file to reveal or
            // preview behind it. The icon comes from the synthetic-row symbol.
            path: "",
            score: .max
        )
        result.meetingProviderLabel = meeting.providerLabel
        result.meetingWhen = Self.detail(meeting, timing: timing)
        return result
    }

    /// The preview pane's line. A countdown is paired with the clock time it is
    /// counting to; a timing that already names the time is left alone, so the
    /// pane never reads "14:30 · tomorrow 14:30".
    private static func detail(_ meeting: JoinableMeeting, timing: String) -> String {
        let clock = clockTime.string(from: meeting.startDate)
        return timing.contains(clock) ? timing : "\(clock)  ·  \(timing)"
    }

    /// When it starts, in the words the row has room for. A meeting under way
    /// says so rather than counting negative minutes, and one that is not today
    /// says WHEN rather than counting to 1440 minutes.
    static func meetingTiming(_ meeting: JoinableMeeting) -> String {
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

    /// "Thu" - enough to place a meeting inside the two-day window the service
    /// looks over.
    private static let weekday: DateFormatter = {
        let formatter = DateFormatter()
        formatter.setLocalizedDateFormatFromTemplate("EEE")
        return formatter
    }()

    /// The start time in the user's own clock format, for the preview pane. The
    /// row itself only has room for the countdown.
    private static let clockTime: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()
}
