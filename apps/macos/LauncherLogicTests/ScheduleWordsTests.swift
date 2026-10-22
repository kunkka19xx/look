import XCTest
@testable import LauncherLogic

/// The gates deciding whether a question sees the user's calendar, and which
/// store it means. Wrong answers here are user-visible: a missed gate makes
/// "am I free friday" shrug, an over-eager one sends a pointless EventKit
/// fetch (and, for the answer card, hides the web sources).
final class ScheduleWordsTests: XCTestCase {
    func testScheduleQuestionsAreGated() {
        for query in [
            "what's on my calendar?",
            "am I free friday",
            "am I busy tomorrow",
            "my meetings this week",
            "what are my reminders",
        ] {
            XCTAssertTrue(ScheduleWords.mentionsSchedule(query), query)
        }
    }

    func testNonScheduleQueriesAreNotGated() {
        for query in [
            "how do I list open ports on macOS",
            "what is a monad",
            "spotify",
            "convert 10 usd to eur",
        ] {
            XCTAssertFalse(ScheduleWords.mentionsSchedule(query), query)
        }
    }

    func testReminderQuestionsPreferReminders() {
        XCTAssertTrue(ScheduleWords.prefersReminders("what are my reminders"))
        XCTAssertTrue(ScheduleWords.prefersReminders("show my todos"))
    }

    func testMixedQuestionsPreferEvents() {
        // Naming both means the calendar answer, not the reminder list.
        XCTAssertFalse(ScheduleWords.prefersReminders("my meetings and reminders today"))
        XCTAssertFalse(ScheduleWords.prefersReminders("what's on my calendar"))
    }

    func testMatchingIsWholeWordAndCaseInsensitive() {
        // Substring matching would fire on "freedom" (free) and "weekly" (week).
        XCTAssertFalse(ScheduleWords.mentionsSchedule("freedom of the press"))
        XCTAssertFalse(ScheduleWords.mentionsSchedule("weekly.log"))
        XCTAssertTrue(ScheduleWords.mentionsSchedule("What's on my CALENDAR?"))
    }
}
