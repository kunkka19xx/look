import Foundation

/// The pinned "Join <meeting>" row. The grammar ("is this a join request?"),
/// the link hiding in the invite, and the choice of WHICH meeting all live in
/// the shared `core/ai` crate via `EngineBridge` and `MeetingService`, so this
/// file is presentation and placement only. Mirrors `LauncherView+Calc.swift`.
extension LauncherView {
    private enum Copy {
        static let enterHint = "Enter to join"
    }

    /// Open the highlighted picker row and get out of the way. The meeting app,
    /// the browser, or FaceTime is taking the screen, so a launcher left open on
    /// a status bar is a dead end the user has to Esc out of. A failure keeps
    /// the panel up, since that is the case with something to read.
    func openHighlightedLink() {
        guard actionController.openSelectedLink() else { return }
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

        let timing = MeetingTiming.phrase(meeting)
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
        result.linkKindLabel = meeting.providerLabel
        result.linkDetail = Self.detail(meeting, timing: timing)
        return result
    }

    /// The preview pane's line. A countdown is paired with the clock time it is
    /// counting to; a timing that already names the time is left alone, so the
    /// pane never reads "14:30 · tomorrow 14:30".
    private static func detail(_ meeting: JoinableMeeting, timing: String) -> String {
        let clock = MeetingTiming.clockTime.string(from: meeting.startDate)
        return timing.contains(clock) ? timing : "\(clock)  ·  \(timing)"
    }
}
