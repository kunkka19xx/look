import Foundation

/// The pinned "Join <meeting>" row. The grammar ("is this a join request?"),
/// the link hiding in the invite, and the choice of WHICH meeting all live in
/// the shared `core/ai` crate via `EngineBridge` and `MeetingService`, so this
/// file is presentation and placement only. Mirrors `LauncherView+Calc.swift`.
extension LauncherView {
    private enum Copy {
        static let inProgress = "in progress"
        static let startingNow = "starting now"
        static let enterHint = "Enter to join"
    }

    /// A synthesized row for `join`-style queries, or nil. The calendar read
    /// behind this is cached (see `MeetingService`), so it is safe to evaluate
    /// on every keystroke like the other pinned rows.
    var meetingResult: LauncherResult? {
        guard allowsSuggestionRows, bridge.isJoinQuery(query) else { return nil }
        guard let meeting = MeetingService.shared.nextMeeting() else { return nil }

        let timing = Self.timing(meeting)
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
        result.meetingWhen = "\(Self.clockTime.string(from: meeting.startDate))  ·  \(timing)"
        return result
    }

    /// When it starts, in the words the row has room for. A meeting under way
    /// says so rather than counting negative minutes.
    private static func timing(_ meeting: JoinableMeeting) -> String {
        if meeting.inProgress { return Copy.inProgress }
        let minutes = meeting.minutesUntilStart
        guard minutes > 0 else { return Copy.startingNow }
        return "in \(minutes) min"
    }

    /// The start time in the user's own clock format, for the preview pane. The
    /// row itself only has room for the countdown.
    private static let clockTime: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()
}
