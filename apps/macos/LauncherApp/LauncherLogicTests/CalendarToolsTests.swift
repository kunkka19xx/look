import XCTest
@testable import LauncherLogic

/// In-memory `EventStoring` for tool tests. No EventKit, no permission.
final class FakeStore: EventStoring {
    private(set) var events: [String: (title: String, start: Date, end: Date, isAllDay: Bool)] = [:]
    private(set) var reminders: [String: (title: String, due: Date?)] = [:]
    private var counter = 0

    func addEvent(title: String, start: Date, end: Date, isAllDay: Bool) throws -> String {
        counter += 1
        let id = "e\(counter)"
        events[id] = (title, start, end, isAllDay)
        return id
    }

    func removeEvent(id: String) throws { events[id] = nil }

    func addReminder(title: String, due: Date?) throws -> String {
        counter += 1
        let id = "r\(counter)"
        reminders[id] = (title, due)
        return id
    }

    func removeReminder(id: String) throws { reminders[id] = nil }
}

final class CalendarToolsTests: XCTestCase {
    private let now = Date(timeIntervalSince1970: 1_754_000_000)  // fixed reference

    // ── calendar.add_event ──────────────────────────────────────────────

    func testAddEventPlansCreatesAndUndoes() throws {
        let store = FakeStore()
        let tool = CalendarAddEventTool(store: store)
        let result = tool.plan(
            ["title": .string("Dentist"), "when": .string("January 2 2027 9am")], now: now)

        guard case .planned(let action) = result else { return XCTFail("expected planned") }
        XCTAssertEqual(action.preview.title, "Add event")
        XCTAssertTrue(action.preview.detail.contains("Dentist"))

        let receipt = try action.perform()
        XCTAssertEqual(store.events.count, 1)
        XCTAssertEqual(store.events.values.first?.title, "Dentist")
        XCTAssertLessThan(store.events.values.first!.start, store.events.values.first!.end)

        try receipt.undo()
        XCTAssertTrue(store.events.isEmpty)
    }

    func testAddEventDefaultsToSixtyMinutes() throws {
        let store = FakeStore()
        let tool = CalendarAddEventTool(store: store)
        guard case .planned(let action) = tool.plan(
            ["title": .string("X"), "when": .string("January 2 2027 9am")], now: now)
        else { return XCTFail("expected planned") }
        _ = try action.perform()
        let event = store.events.values.first!
        XCTAssertEqual(event.end.timeIntervalSince(event.start), 3600)
    }

    func testAddEventMissingTitleIsInvalid() {
        let tool = CalendarAddEventTool(store: FakeStore())
        guard case .invalid = tool.plan(["when": .string("5pm")], now: now) else {
            return XCTFail("expected invalid")
        }
    }

    // ── all-day events ──────────────────────────────────────────────────

    func testDayWithoutClockTimeIsAllDay() throws {
        let store = FakeStore()
        let tool = CalendarAddEventTool(store: store)
        guard case .planned(let action) = tool.plan(
            ["title": .string("Birthday"), "when": .string("March 5 2027")], now: now)
        else { return XCTFail("expected planned") }
        XCTAssertTrue(action.preview.detail.contains("all day"))
        _ = try action.perform()
        XCTAssertEqual(store.events.values.first?.isAllDay, true)
    }

    func testNoDateDefaultsToAllDayToday() throws {
        let store = FakeStore()
        let tool = CalendarAddEventTool(store: store)
        guard case .planned(let action) = tool.plan(["title": .string("Lunch with Sarah")], now: now)
        else { return XCTFail("expected planned") }
        _ = try action.perform()
        let event = store.events.values.first!
        XCTAssertTrue(event.isAllDay)
        XCTAssertEqual(Calendar.current.startOfDay(for: now), event.start)
    }

    func testDayWithClockTimeIsTimed() throws {
        let store = FakeStore()
        let tool = CalendarAddEventTool(store: store)
        guard case .planned(let action) = tool.plan(
            ["title": .string("Sync"), "when": .string("March 5 2027 3pm")], now: now)
        else { return XCTFail("expected planned") }
        _ = try action.perform()
        XCTAssertEqual(store.events.values.first?.isAllDay, false)
    }

    func testHasClockTimeDetection() {
        XCTAssertTrue(DatePhrase.hasClockTime("3pm"))
        XCTAssertTrue(DatePhrase.hasClockTime("friday 15:00"))
        XCTAssertTrue(DatePhrase.hasClockTime("friday noon"))
        XCTAssertFalse(DatePhrase.hasClockTime("march 5"))
        XCTAssertFalse(DatePhrase.hasClockTime("next friday"))
    }

    func testAddEventUnparseableTimeIsInvalid() {
        let tool = CalendarAddEventTool(store: FakeStore())
        guard case .invalid = tool.plan(
            ["title": .string("X"), "when": .string("qwerty")], now: now)
        else { return XCTFail("expected invalid") }
    }

    // ── reminder.add ────────────────────────────────────────────────────

    func testAddReminderWithoutDuePlansAndUndoes() throws {
        let store = FakeStore()
        let tool = ReminderAddTool(store: store)
        guard case .planned(let action) = tool.plan(["title": .string("Buy milk")], now: now)
        else { return XCTFail("expected planned") }

        let receipt = try action.perform()
        XCTAssertEqual(store.reminders.count, 1)
        XCTAssertNil(store.reminders.values.first!.due)

        try receipt.undo()
        XCTAssertTrue(store.reminders.isEmpty)
    }

    func testAddReminderInvalidTimeIsInvalid() {
        let tool = ReminderAddTool(store: FakeStore())
        guard case .invalid = tool.plan(
            ["title": .string("X"), "when": .string("qwerty")], now: now)
        else { return XCTFail("expected invalid") }
    }
}
