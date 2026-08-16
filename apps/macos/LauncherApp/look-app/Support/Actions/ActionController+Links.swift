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
    /// Joinable meetings for the user to pick from. Returns feedback only when
    /// there is nothing to show.
    func presentJoinChoices(named name: String?, didAsk: Bool = false) -> String {
        // No access reads downstream as "no meetings", so settle it first.
        switch EventKitService.shared.calendarAccess {
        case .authorized:
            break
        case .notDetermined:
            // Once only: a failed request leaves the status unchanged, and
            // retrying on that loops.
            guard !didAsk else { return Self.noCalendarAccess }
            Task {
                await EventKitService.shared.requestCalendarAccess()
                setFeedback(presentJoinChoices(named: name, didAsk: true))
            }
            return ""
        // Write-only cannot read events.
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

    /// One meeting as a picker row.
    private static func option(_ meeting: JoinableMeeting) -> LinkOption {
        var detail = [meeting.providerLabel, MeetingTiming.phrase(meeting)]
        if let host = URL(string: meeting.url)?.host { detail.append(host) }
        return LinkOption(
            // Not the URL alone: a personal room and a recurring series
            // repeat it, and duplicate ids break row identity.
            id: "\(meeting.startUnixS)|\(meeting.url)",
            title: meeting.title,
            detail: detail.joined(separator: "  ·  "),
            symbol: "video.fill",
            url: meeting.url)
    }

    /// Ways to reach the person the user named. Always lists, even for one
    /// option: a call has no undo, so the row the user reads is the confirm.
    func presentCallChoices(named name: String, modality: String?, didAsk: Bool = false)
        -> String
    {
        switch ContactsService.shared.access {
        case .authorized:
            break
        case .notDetermined:
            // Once only: see the calendar branch.
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

        // The verb did not say how.
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
            // Found the person, but no handle for this modality.
            return "\u{201C}\(matches[0].name)\u{201D} has no number or address for that."
        }

        setLinkPicker(
            LinkPicker(
                header: matches.count > 1 ? "Reach who?" : "Reach how?",
                options: options,
                selected: 0))
        return ""
    }

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

    private static let noCalendarAccess =
        "Look has no calendar access, so it cannot see your meetings. "
        + "Grant it in Settings (\u{2318}\u{21E7},) under Permissions."

    /// Why there was nothing to open. A meeting with no link is named, since
    /// that is a different problem from having no such meeting.
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

    /// Opens the highlighted row. Returns whether it opened, so the caller can
    /// hide the launcher. Success leaves no feedback: a sticky bar would block
    /// the sessions list, which reads feedback as "busy".
    @discardableResult
    func openSelectedLink() -> Bool {
        guard let picker = linkPicker, let option = picker.selectedOption else { return false }
        // Cleared only on success, so a failed open leaves the list to retry.
        guard let url = URL(string: option.url), NSWorkspace.shared.open(url) else {
            setFeedback("Could not open \(option.title).")
            return false
        }
        setLinkPicker(nil)
        setFeedback("")
        return true
    }

    /// Tab / arrows roll the picker. False when none is up, so the key falls
    /// through.
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

    /// Moves the highlight to a 1-based position. False when the number names
    /// no row, so it stays an ordinary message.
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
