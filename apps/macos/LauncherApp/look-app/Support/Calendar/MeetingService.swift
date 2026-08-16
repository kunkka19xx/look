import AppKit
import Foundation

/// One event, flattened to exactly the fields the Rust core needs to find a
/// join link and pick between meetings. Keys match `look_ai::meeting::EventInput`.
nonisolated struct MeetingEventPayload: Encodable {
    let title: String
    let startUnixS: Int64
    let endUnixS: Int64
    let url: String?
    let location: String?
    let notes: String?
    let allDay: Bool
}

/// What a `join` request turned up. Mirrors `look_ai::meeting::JoinOutcome`.
nonisolated struct JoinOutcome: Decodable, Equatable {
    var meetings: [JoinableMeeting] = []
    /// Titles that matched the name but carry no join link, so the answer can
    /// say which meeting is missing one rather than claiming none exists.
    var withoutLink: [String] = []
}

/// A parsed `join ...` request. `name` is the words that were not filler, so
/// "join testing" carries "testing" and a bare "join" carries nothing.
nonisolated struct JoinRequest: Decodable, Equatable {
    var name: String?
}

/// The meeting to join, as decided in core. Mirrors
/// `look_ai::meeting::JoinableMeeting`.
nonisolated struct JoinableMeeting: Decodable, Equatable {
    let title: String
    let startUnixS: Int64
    let endUnixS: Int64
    let url: String
    /// Provider id (`teams`, `zoom`, `meet`, ...). `providerLabel` is what to show.
    let provider: String
    let providerLabel: String
    /// Negative once the meeting has started.
    let startsInS: Int64
    let inProgress: Bool

    var startDate: Date { Date(timeIntervalSince1970: TimeInterval(startUnixS)) }
    var joinURL: URL? { URL(string: url) }

    /// Rounded up, so a meeting 61 seconds out reads "in 2 min" rather than
    /// "in 1 min" for most of the minute it is counting down.
    var minutesUntilStart: Int {
        Int((Double(startsInS) / 60.0).rounded(.up))
    }
}

/// "Join my next meeting": read the calendar, let core find the link, open it.
///
/// Look makes no network call here. A Teams, Zoom, or Meet invite already
/// carries its join URL, and the account sync that put it there is the OS's
/// job (see docs/ai-eventkit-connector.md).
nonisolated final class MeetingService: @unchecked Sendable {
    static let shared = MeetingService()

    private enum Metrics {
        /// How far ahead to look for something to join. Two days, not twelve
        /// hours: "my next meeting" on a Friday evening is Monday's, and a
        /// window that quietly excludes tomorrow reads as the feature being
        /// broken rather than as a policy.
        static let lookahead: TimeInterval = 48 * 60 * 60
        /// A meeting that started a while ago is still joinable, so the window
        /// opens slightly behind now. Core drops anything already ended.
        static let lookbehind: TimeInterval = -60 * 60
        /// Mirrors the event cache in `EventKitService`. The join row is a
        /// COMPUTED property of the launcher view, so SwiftUI re-reads it on
        /// every update; without this, one keystroke would mean several
        /// EventKit fetches.
        static let cacheTTL: TimeInterval = 5
    }

    private let lock = NSLock()
    private var cached = JoinOutcome()
    private var cachedName = ""
    private var cachedAt = Date.distantPast

    private init() {}

    /// What a `join` finds: the joinable meetings, best first, and the titles
    /// that matched but carry no link. `name` narrows to meetings whose title
    /// holds those words.
    ///
    /// Cached for `Metrics.cacheTTL`; the countdown shown is derived from each
    /// meeting's own start time, so a cached answer is not a stale one. Keyed
    /// on the name too, since typing "join st" then "join standup" asks two
    /// different questions inside one TTL.
    func outcome(name: String = "", now: Date = Date()) -> JoinOutcome {
        lock.lock()
        defer { lock.unlock() }
        if name == cachedName, now.timeIntervalSince(cachedAt) < Metrics.cacheTTL {
            return cached
        }
        cachedAt = now
        cachedName = name
        cached = fetchOutcome(name: name, now: now)
        return cached
    }

    /// Every meeting that could be joined, best first.
    func meetings(name: String = "", now: Date = Date()) -> [JoinableMeeting] {
        outcome(name: name, now: now).meetings
    }

    /// The one a bare "join" would take: the head of the list.
    func nextMeeting(name: String = "", now: Date = Date()) -> JoinableMeeting? {
        meetings(name: name, now: now).first
    }

    /// Drop the cache, for when the calendar changed under us.
    func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        cachedAt = .distantPast
        cached = JoinOutcome()
    }

    private func fetchOutcome(name: String, now: Date) -> JoinOutcome {
        let events = EventKitService.shared.meetingEventPayloads(
            from: now.addingTimeInterval(Metrics.lookbehind),
            to: now.addingTimeInterval(Metrics.lookahead))
        guard !events.isEmpty else { return JoinOutcome() }
        guard let json = try? JSONEncoder().encode(events),
            let jsonString = String(data: json, encoding: .utf8)
        else { return JoinOutcome() }
        return EngineBridge.shared.joinOutcome(
            eventsJSON: jsonString, now: Int64(now.timeIntervalSince1970), name: name)
    }

    /// Opens the join link. The https form is deliberate: it reaches the
    /// desktop app through universal links when installed, and the browser when
    /// not, where `msteams:` / `zoommtg:` would fail silently.
    @discardableResult
    func join(_ meeting: JoinableMeeting) -> Bool {
        guard let url = meeting.joinURL else { return false }
        return NSWorkspace.shared.open(url)
    }
}
