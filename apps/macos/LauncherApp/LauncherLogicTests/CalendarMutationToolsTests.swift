import XCTest
@testable import LauncherLogic

/// The Phase B safety core: the match gate (confident / choice / none), the
/// chosen_id bypass, and faithful undo for cancel (recreate), move (restore),
/// and complete (uncomplete). All against the FakeStore, fixed `now`.
final class CalendarMutationToolsTests: XCTestCase {
    private let now = Date(timeIntervalSince1970: 1_754_000_000)

    private func seededStore() throws -> (FakeStore, dentist: String, sync: String, standup: String) {
        let store = FakeStore()
        let dentist = try store.addEvent(
            title: "Dentist", start: now.addingTimeInterval(3600),
            end: now.addingTimeInterval(7200), isAllDay: false)
        let sync = try store.addEvent(
            title: "Team Sync", start: now.addingTimeInterval(86_400),
            end: now.addingTimeInterval(90_000), isAllDay: false)
        let standup = try store.addEvent(
            title: "Team Standup", start: now.addingTimeInterval(172_800),
            end: now.addingTimeInterval(176_400), isAllDay: false)
        return (store, dentist, sync, standup)
    }

    // ── TitleMatcher gate ───────────────────────────────────────────────

    func testMatcherTiers() {
        XCTAssertEqual(TitleMatcher.score(query: "dentist", title: "Dentist"), 1000)
        XCTAssertEqual(TitleMatcher.score(query: "sync", title: "Team Sync"), 500)
        XCTAssertEqual(TitleMatcher.score(query: "te sy", title: "Team Sync"), 300)
        XCTAssertNil(TitleMatcher.score(query: "zebra", title: "Team Sync"))
    }

    // ── referent phrases ("this event", "it") ───────────────────────────

    func testReferentPhrases() {
        XCTAssertTrue(ReferentPhrase.isReferent("it"))
        XCTAssertTrue(ReferentPhrase.isReferent("this event"))
        XCTAssertTrue(ReferentPhrase.isReferent("that meeting"))
        XCTAssertTrue(ReferentPhrase.isReferent("the last one"))
        XCTAssertFalse(ReferentPhrase.isReferent("dentist"))
        XCTAssertFalse(ReferentPhrase.isReferent("this dentist visit"))
        XCTAssertFalse(ReferentPhrase.isReferent("the meeting"))  // names, not refers
    }

    // ── cancel ──────────────────────────────────────────────────────────

    func testCancelConfidentMatchPerformsAndUndoRecreates() throws {
        let (store, dentistID, _, _) = try seededStore()
        let tool = CalendarCancelEventTool(store: store)

        guard case .planned(let action) = tool.plan(["match": .string("dentist")], now: now)
        else { return XCTFail("expected planned") }
        XCTAssertTrue(action.preview.detail.contains("Dentist"))

        let receipt = try action.perform()
        XCTAssertNil(store.events[dentistID])

        try receipt.undo()
        XCTAssertTrue(store.events.values.contains { $0.title == "Dentist" })
    }

    func testCancelAmbiguousMatchNeedsChoice() throws {
        let (store, _, syncID, standupID) = try seededStore()
        let tool = CalendarCancelEventTool(store: store)

        guard case .needsChoice(let options) = tool.plan(["match": .string("team")], now: now)
        else { return XCTFail("expected needsChoice") }
        XCTAssertEqual(Set(options.map(\.id)), [syncID, standupID])
        // The gate held: nothing was mutated.
        XCTAssertEqual(store.events.count, 3)
    }

    func testCancelNoMatchIsInvalid() throws {
        let (store, _, _, _) = try seededStore()
        guard case .invalid = CalendarCancelEventTool(store: store)
            .plan(["match": .string("zebra")], now: now)
        else { return XCTFail("expected invalid") }
    }

    func testCancelChosenIDBypassesMatching() throws {
        let (store, _, syncID, _) = try seededStore()
        let tool = CalendarCancelEventTool(store: store)
        guard case .planned(let action) = tool.plan(["chosen_id": .string(syncID)], now: now)
        else { return XCTFail("expected planned") }
        _ = try action.perform()
        XCTAssertNil(store.events[syncID])
    }

    // ── move ────────────────────────────────────────────────────────────

    func testMovePreservesDurationAndUndoRestores() throws {
        let (store, dentistID, _, _) = try seededStore()
        let tool = CalendarMoveEventTool(store: store)
        let oldStart = store.events[dentistID]!.start

        guard case .planned(let action) = tool.plan(
            ["match": .string("dentist"), "when": .string("January 9 2027 3pm")], now: now)
        else { return XCTFail("expected planned") }

        let receipt = try action.perform()
        let moved = store.events[dentistID]!
        XCTAssertEqual(moved.end.timeIntervalSince(moved.start), 3600)  // duration kept
        XCTAssertNotEqual(moved.start, oldStart)
        XCTAssertEqual(receipt.subjectID, dentistID)

        try receipt.undo()
        XCTAssertEqual(store.events[dentistID]!.start, oldStart)
    }

    func testMoveDayOnlyPhraseKeepsClockTime() throws {
        let (store, dentistID, _, _) = try seededStore()
        let tool = CalendarMoveEventTool(store: store)
        let oldClock = Calendar.current.dateComponents(
            [.hour, .minute], from: store.events[dentistID]!.start)

        guard case .planned(let action) = tool.plan(
            ["match": .string("dentist"), "when": .string("January 9 2027")], now: now)
        else { return XCTFail("expected planned") }
        _ = try action.perform()

        let newClock = Calendar.current.dateComponents(
            [.hour, .minute], from: store.events[dentistID]!.start)
        XCTAssertEqual(newClock, oldClock)
    }

    // ── complete reminder ───────────────────────────────────────────────

    func testCompleteReminderAndUndo() throws {
        let store = FakeStore()
        let id = try store.addReminder(title: "Call plumber", due: nil)
        let tool = ReminderCompleteTool(store: store)

        guard case .planned(let action) = tool.plan(["match": .string("plumber")], now: now)
        else { return XCTFail("expected planned") }
        let receipt = try action.perform()
        XCTAssertTrue(store.reminders[id]!.completed)
        XCTAssertEqual(receipt.subjectID, id)

        try receipt.undo()
        XCTAssertFalse(store.reminders[id]!.completed)
    }

    func testCompletedRemindersAreNotCandidates() throws {
        let store = FakeStore()
        let id = try store.addReminder(title: "Call plumber", due: nil)
        try store.completeReminder(id: id)
        guard case .invalid = ReminderCompleteTool(store: store)
            .plan(["match": .string("plumber")], now: now)
        else { return XCTFail("expected invalid") }
    }
}
