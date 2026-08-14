import Foundation

/// Turns a schedule-shaped question into a calendar/reminder listing, scoped by
/// the window grammar ("next week" -> that range). One place for it, because
/// three surfaces need the same answer: the chat prompt's context block, the
/// main-bar answer card, and the deterministic no-model fallback.
///
/// Local only: this text goes to the on-machine model or straight to the UI.
@MainActor
enum ScheduleContextProvider {
    struct Listing {
        let summary: String
        /// Human scope for the sentence ("for next week", "reminders").
        let label: String
        /// What was listed, so the caller can make it the referent set
        /// ("remove this event" right after seeing the list).
        let events: [EventCandidateData]
        let reminders: [ReminderCandidateData]
    }

    static func mentionsSchedule(_ query: String) -> Bool {
        ScheduleWords.mentionsSchedule(query)
    }

    /// The listing a question asks for, or nil without read access.
    static func listing(for query: String) -> Listing? {
        if ScheduleWords.prefersReminders(query) {
            guard let (summary, reminders) = EventKitService.shared.remindersSummary() else {
                return nil
            }
            return Listing(summary: summary, label: "reminders", events: [], reminders: reminders)
        }
        if let window = EngineBridge.shared.aiQueryWindow(query) {
            guard let (summary, events) = EventKitService.shared.eventsSummary(
                from: window.start, to: window.end,
                emptyText: "No events \(window.label).")
            else { return nil }
            return Listing(
                summary: summary, label: "for \(window.label)", events: events, reminders: [])
        }
        guard let (summary, events) = EventKitService.shared.upcomingEventsSummary() else {
            return nil
        }
        return Listing(
            summary: summary, label: "for the next 7 days", events: events, reminders: [])
    }

    /// The system-prompt block for a chat turn. Without access the model is told
    /// to point at Settings rather than failing mysteriously.
    static func chatContext(for query: String, listing: Listing?) -> String {
        guard let listing else {
            return "You cannot see the user's calendar because access is not "
                + "granted. Tell them to connect Calendar via the Permissions "
                + "row in Look's Settings."
        }
        let df = DateFormatter()
        df.dateFormat = "EEEE, MMMM d yyyy, HH:mm"
        return "Now: \(df.string(from: Date())). "
            + "The user's calendar \(listing.label):\n\(listing.summary)"
    }

    /// Answer for the main-bar card: the listing IS the answer, so no model is
    /// involved (and a personal question never reaches the web sources).
    static func cardAnswer(for query: String) -> (text: String, source: String)? {
        guard mentionsSchedule(query) else { return nil }
        guard let listing = listing(for: query) else {
            return (
                "Connect Calendar via the Permissions row in Look's Settings "
                    + "to answer schedule questions.",
                "Calendar"
            )
        }
        ActionController.shared.rememberListed(listing)
        return ("Your calendar \(listing.label):\n\(listing.summary)", "Calendar")
    }
}
