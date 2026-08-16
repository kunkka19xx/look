import AppKit
import Foundation

/// The two tiers that end in "open a URL from a list": `join` a meeting and
/// `call` a person.
///
/// Split out of `ActionController` because they share a shape and nothing else
/// does: resolve a name against a platform store (EventKit, Contacts), settle
/// access first so "cannot see" never reads as "nothing found", and hand back
/// a `LinkPicker` for the user to choose from.
extension ActionController {
    /// Puts the joinable meetings on screen for the user to pick from.
    ///
    /// It LISTS rather than joining outright, even when there is only one
    /// candidate: "join" resolves to a specific meeting at a specific time, and
    /// opening it before the user has seen which one gives them no way to
    /// notice it picked the wrong thing. Returns feedback only when there is
    /// nothing to show.
    func presentJoinChoices(named name: String?, didAsk: Bool = false) -> String {
        // Without access the calendar reads as empty, which downstream is
        // indistinguishable from "you have no meetings" - and saying that when
        // Look simply cannot see them sends the user hunting for the wrong
        // problem. Settle access FIRST, and say so in its own words.
        switch EventKitService.shared.calendarAccess {
        case .authorized:
            break
        case .notDetermined:
            // Once only. `requestCalendarAccess` discards its error, so a
            // failed request leaves the status unchanged - retrying on that
            // would spawn a task that requests again, forever.
            guard !didAsk else { return Self.noCalendarAccess }
            // A TCC prompt is one-shot, so it belongs at the moment the user
            // asked for something that needs it, not at launch.
            Task {
                await EventKitService.shared.requestCalendarAccess()
                setFeedback(presentJoinChoices(named: name, didAsk: true))
            }
            return ""
        // Write-only can add events but cannot READ them, so there is nothing
        // to join with it either.
        case .writeOnly, .denied, .restricted:
            setLinkPicker(nil)
            return Self.noCalendarAccess
        }

        let wanted = name ?? ""
        let outcome = MeetingService.shared.outcome(name: wanted)
        guard !outcome.meetings.isEmpty else {
            setLinkPicker(nil)
            return Self.nothingToJoin(wanted: wanted, withoutLink: outcome.withoutLink)
        }
        setLinkPicker(
            LinkPicker(header: "Join which?", options: outcome.meetings.map(Self.option), selected: 0))
        return ""
    }

    /// One meeting as a picker row. The detail line is what the row promises:
    /// which service, when, and the host the link will actually open.
    private static func option(_ meeting: JoinableMeeting) -> LinkOption {
        var detail = [meeting.providerLabel, MeetingTiming.phrase(meeting)]
        if let host = URL(string: meeting.url)?.host { detail.append(host) }
        return LinkOption(
            id: meeting.url,
            title: meeting.title,
            detail: detail.joined(separator: "  ·  "),
            symbol: "video.fill",
            url: meeting.url)
    }

    /// Ways to reach the person the user named.
    ///
    /// ALWAYS lists, even for a single option. An earlier version dialled
    /// straight through when there was only one way to reach someone, on the
    /// theory that confirming what you just typed is noise - but from the
    /// user's side nothing appeared in the launcher and then FaceTime started
    /// ringing a person. Placing a call is socially expensive and has no undo,
    /// so the row the user reads IS the confirmation.
    func presentCallChoices(named name: String, modality: String?, didAsk: Bool = false)
        -> String
    {
        switch ContactsService.shared.access {
        case .authorized:
            break
        case .notDetermined:
            // Once only, for the same reason as the calendar branch above.
            guard !didAsk else { return Self.noContactsAccess }
            Task {
                await ContactsService.shared.requestAccess()
                setFeedback(presentCallChoices(named: name, modality: modality, didAsk: true))
            }
            return ""
        case .writeOnly, .denied, .restricted:
            setLinkPicker(nil)
            return Self.noContactsAccess
        }

        // The verb said "call" but not how, so this is the house default. Kept
        // out of core: which one is right depends on what the platform can do.
        let wantedModality = modality ?? EngineBridge.shared.defaultCallModality
        let matches = ContactsService.shared.matches(name: name)
        let options: [LinkOption] = matches.flatMap { match in
            match.handles
                .filter { $0.modalityID == wantedModality }
                .compactMap { handle in Self.option(match: match, handle: handle) }
        }

        guard !options.isEmpty else {
            setLinkPicker(nil)
            guard !matches.isEmpty else {
                return "No contact matching \u{201C}\(name)\u{201D}."
            }
            // Found the person, but not a handle that does THIS. Naming the
            // gap beats "no contact", which would send them looking for the
            // wrong problem.
            return "\u{201C}\(matches[0].name)\u{201D} has no number or address for that."
        }
        // Several people is a "who"; one person with several numbers is a
        // "how". Both are the same list, but the question is not.
        setLinkPicker(
            LinkPicker(
                header: matches.count > 1 ? "Reach who?" : "Reach how?",
                options: options,
                selected: 0))
        return ""
    }

    /// One way to reach one person, as a picker row.
    private static func option(match: ContactMatch, handle: ContactHandle) -> LinkOption? {
        guard let url = EngineBridge.shared.callURL(modality: handle.modalityID, handle: handle.handle)
        else { return nil }
        var detail = [handle.modalityLabel]
        if let label = handle.handleLabel, !label.isEmpty { detail.append(label) }
        detail.append(handle.handle)
        return LinkOption(
            id: "\(match.id)|\(handle.id)",
            title: match.name,
            detail: detail.joined(separator: "  ·  "),
            symbol: handle.modalityID == "message" ? "message.fill" : "video.fill",
            url: url)
    }

    private static let noContactsAccess =
        "Look has no contacts access, so it cannot find who you mean. "
        + "Grant it in Settings (\u{2318}\u{21E7},) under Permissions."

    /// Names the actual blocker and where to fix it. Not "no meetings": Look
    /// has not looked.
    private static let noCalendarAccess =
        "Look has no calendar access, so it cannot see your meetings. "
        + "Grant it in Settings (\u{2318}\u{21E7},) under Permissions."

    /// Why there was nothing to open. A meeting that IS on the calendar but
    /// carries no conferencing link gets named: "no meeting matching Testing"
    /// reads as "you have no such meeting", which is a different, wrong
    /// problem to go looking for.
    private static func nothingToJoin(wanted: String, withoutLink: [String]) -> String {
        let where_ = "Add a Zoom, Teams, or Meet link to its URL, location, or notes."
        switch withoutLink.count {
        case 0:
            guard !wanted.isEmpty else { return "No meeting to join in the next two days." }
            return "No meeting matching \u{201C}\(wanted)\u{201D} in the next two days."
        case 1:
            return "\u{201C}\(withoutLink[0])\u{201D} has no meeting link. \(where_)"
        default:
            let named = withoutLink.map { "\u{201C}\($0)\u{201D}" }.joined(separator: ", ")
            return "No join link on \(named). \(where_)"
        }
    }

    /// Opens the highlighted row. No confirm step beyond this and no undo,
    /// unlike the calendar tools: the row the user just read IS the
    /// confirmation, and opening a link reverses nothing.
    ///
    /// Returns whether it opened, so the caller can put the launcher away.
    /// Success leaves NO feedback behind: the meeting or the call is on screen
    /// by then, and a sticky "Joining ..." bar would sit on the panel forever
    /// (it also blocks the sessions list, which reads feedback as "busy").
    /// A failure is the one case worth leaving up to read.
    @discardableResult
    func openSelectedLink() -> Bool {
        guard let picker = linkPicker, let option = picker.selectedOption else { return false }
        // Cleared only on success: if the open fails, the list stays up so the
        // user can try another row instead of retyping the whole request.
        guard let url = URL(string: option.url), NSWorkspace.shared.open(url) else {
            setFeedback("Could not open \(option.title).")
            return false
        }
        setLinkPicker(nil)
        setFeedback("")
        return true
    }

    /// Tab / arrows roll the picker. Returns false when none is up, so the key
    /// falls through to whatever else the panel is showing.
    @discardableResult
    func movePickerSelection(forward: Bool) -> Bool {
        guard var picker = linkPicker, !picker.options.isEmpty else { return false }
        let count = picker.options.count
        picker.selected =
            forward
            ? (picker.selected >= count - 1 ? 0 : picker.selected + 1)
            : (picker.selected <= 0 ? count - 1 : picker.selected - 1)
        setLinkPicker(picker)
        return true
    }

    /// Moves the highlight to a 1-based position, matching what the rows show.
    /// Returns false when the number names no row, so a typed "7" with three
    /// rows listed stays an ordinary message rather than doing nothing.
    @discardableResult
    func selectPickerRow(number: Int) -> Bool {
        guard var picker = linkPicker, number >= 1, number <= picker.options.count else {
            return false
        }
        picker.selected = number - 1
        setLinkPicker(picker)
        return true
    }

    func clearPicker() { setLinkPicker(nil) }
}
