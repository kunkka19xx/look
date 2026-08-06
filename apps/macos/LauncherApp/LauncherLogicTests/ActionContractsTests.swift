import XCTest
@testable import LauncherLogic

final class ActionContractsTests: XCTestCase {
    // ── AIValue codec ───────────────────────────────────────────────────

    func testAIValueRoundTrips() throws {
        let value: AIValue = .object([
            "title": .string("Dentist"),
            "duration": .number(60),
            "allDay": .bool(false),
            "tags": .array([.string("health"), .null]),
        ])
        let data = try JSONEncoder().encode(value)
        let decoded = try JSONDecoder().decode(AIValue.self, from: data)
        XCTAssertEqual(decoded, value)
    }

    func testAIValueDecodesBoolAsBoolNotNumber() throws {
        let decoded = try JSONDecoder().decode(AIValue.self, from: Data("true".utf8))
        XCTAssertEqual(decoded, .bool(true))
    }

    func testAIValueDecodesNumber() throws {
        let decoded = try JSONDecoder().decode(AIValue.self, from: Data("42".utf8))
        XCTAssertEqual(decoded, .number(42))
    }

    func testPlanStepDecodesFromWireFormat() throws {
        let json = #"{"steps":[{"tool":"calendar.add_event","params":{"title":"X","when":"5pm"}}]}"#
        let plan = try JSONDecoder().decode(ActionPlan.self, from: Data(json.utf8))
        XCTAssertEqual(plan.steps.count, 1)
        XCTAssertEqual(plan.steps[0].toolCall.toolID, "calendar.add_event")
        XCTAssertEqual(plan.steps[0].params["title"], .string("X"))
    }

    // ── Registry ────────────────────────────────────────────────────────

    func testRegistryRegistersAndLooksUp() {
        let registry = ActionRegistry()
        registry.register(CalendarAddEventTool(store: FakeStore()))
        XCTAssertNotNil(registry.tool(id: "calendar.add_event"))
        XCTAssertEqual(registry.all.count, 1)
    }

    func testRegistryUnknownToolIsInvalid() {
        let registry = ActionRegistry()
        let result = registry.plan(ToolCall(toolID: "nope", params: [:]), now: Date())
        guard case .invalid = result else { return XCTFail("expected .invalid") }
    }

    // ── Explicit `>` parser ─────────────────────────────────────────────

    func testParsesAddWithSeparator() {
        let call = ExplicitActionParser.parse(">add lunch @ tomorrow 12pm")
        XCTAssertEqual(call?.toolID, "calendar.add_event")
        XCTAssertEqual(call?.params["title"], .string("lunch"))
        XCTAssertEqual(call?.params["when"], .string("tomorrow 12pm"))
    }

    func testParsesSeparatorWithoutTrailingSpace() {
        let call = ExplicitActionParser.parse(">add lunch @3pm")
        XCTAssertEqual(call?.toolID, "calendar.add_event")
        XCTAssertEqual(call?.params["title"], .string("lunch"))
        XCTAssertEqual(call?.params["when"], .string("3pm"))
    }

    func testParsesRemindWithSeparator() {
        let call = ExplicitActionParser.parse(">remind call mom @ 5pm")
        XCTAssertEqual(call?.toolID, "reminder.add")
        XCTAssertEqual(call?.params["title"], .string("call mom"))
        XCTAssertEqual(call?.params["when"], .string("5pm"))
    }

    func testDayWordInTitleDefersToModel() {
        // "sunday" before the `@` means the date intent is split; the fast path
        // would silently schedule today. Must defer to the model instead.
        XCTAssertNil(ExplicitActionParser.parse(">remind me to call mom on sunday @ 5pm"))
        XCTAssertNil(ExplicitActionParser.parse(">add standup tomorrow @ 9am"))
        // A clean split stays on the instant path.
        XCTAssertNotNil(ExplicitActionParser.parse(">remind call mom @ sunday 5pm"))
    }

    func testNoModelLenientModeResolvesDayFromWholePhrase() {
        // Without a planner, the fast path must not dead-end: verbatim title,
        // but the `when` becomes the whole phrase so the day resolves right.
        let call = ExplicitActionParser.parse(">add call GF today @2pm", modelAvailable: false)
        XCTAssertEqual(call?.toolID, "calendar.add_event")
        XCTAssertEqual(call?.params["title"], .string("call GF today"))
        XCTAssertEqual(call?.params["when"], .string("call GF today @2pm"))
        // Clean splits keep the exact after-@ phrase even in lenient mode.
        let clean = ExplicitActionParser.parse(">add lunch @ 1pm", modelAvailable: false)
        XCTAssertEqual(clean?.params["when"], .string("1pm"))
    }

    // ── query windows for schedule questions ────────────────────────────

    func testQueryWindowNextWeekStartsAtEndOfThisWeek() {
        let now = Date(timeIntervalSince1970: 1_754_000_000)
        let window = DatePhrase.queryWindow(for: "what's on my calendar next week?", now: now)
        let thisWeek = Calendar.current.dateInterval(of: .weekOfYear, for: now)!
        XCTAssertEqual(window?.start, thisWeek.end)
        XCTAssertEqual(window?.label, "next week")
    }

    func testQueryWindowTomorrowIsOneDay() {
        let now = Date(timeIntervalSince1970: 1_754_000_000)
        let window = DatePhrase.queryWindow(for: "am I busy tomorrow", now: now)!
        let expectedStart = Calendar.current.startOfDay(
            for: Calendar.current.date(byAdding: .day, value: 1, to: now)!)
        XCTAssertEqual(window.start, expectedStart)
        XCTAssertEqual(window.label, "tomorrow")
    }

    func testQueryWindowWeekdayName() {
        let now = Date(timeIntervalSince1970: 1_754_000_000)
        let window = DatePhrase.queryWindow(for: "what's on friday?", now: now)!
        XCTAssertEqual(Calendar.current.component(.weekday, from: window.start), 6)
        XCTAssertEqual(window.label, "Friday")
    }

    func testQueryWindowNilWithoutTimeframe() {
        let now = Date(timeIntervalSince1970: 1_754_000_000)
        XCTAssertNil(DatePhrase.queryWindow(for: "what's on my calendar?", now: now))
    }

    func testQueryWindowComposesUnits() {
        let now = Date(timeIntervalSince1970: 1_754_000_000)
        let cal = Calendar.current

        let nextMonth = DatePhrase.queryWindow(for: "what's on next month", now: now)!
        let expectedMonth = cal.dateInterval(
            of: .month, for: cal.date(byAdding: .month, value: 1, to: now)!)!
        XCTAssertEqual(nextMonth.start, expectedMonth.start)
        XCTAssertEqual(nextMonth.label, "next month")

        let thisYear = DatePhrase.queryWindow(for: "events this year", now: now)!
        XCTAssertEqual(thisYear.start, now)  // clamped: no point listing the past
        XCTAssertEqual(thisYear.end, cal.dateInterval(of: .year, for: now)!.end)

        let inTwoWeeks = DatePhrase.queryWindow(for: "am I busy in 2 weeks", now: now)!
        let expectedWeek = cal.dateInterval(
            of: .weekOfYear, for: cal.date(byAdding: .weekOfYear, value: 2, to: now)!)!
        XCTAssertEqual(inTwoWeeks.start, expectedWeek.start)
        XCTAssertEqual(inTwoWeeks.label, "in 2 weeks")
    }

    func testQueryWindowMonthNames() {
        let now = Date(timeIntervalSince1970: 1_754_000_000)  // early Aug 2025
        let august = DatePhrase.queryWindow(for: "what's happening in august", now: now)!
        XCTAssertEqual(Calendar.current.component(.month, from: august.start), 8)
        XCTAssertEqual(august.label, "August")

        // "may" as a verb must not become the month May.
        let mayVerb = DatePhrase.queryWindow(for: "what may be on tomorrow", now: now)!
        XCTAssertEqual(mayVerb.label, "tomorrow")
    }

    func testNormalizeMapsAtSignToAt() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("sunday @ 5pm"), "sunday at 5pm")
        XCTAssertEqual(DatePhrase.normalizeShorthand("sunday @5pm"), "sunday at 5pm")
    }

    func testNaturalLanguageWithoutSeparatorReturnsNil() {
        // No `@` = natural language, deferred to the model, never guessed here.
        XCTAssertNil(ExplicitActionParser.parse(">remind me to walk my dog at 7pm"))
        XCTAssertNil(ExplicitActionParser.parse(">add lunch tomorrow"))
    }

    // ── Date shorthand normalization ────────────────────────────────────

    func testNormalizesTomorrowShorthand() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("1pm tmr"), "1pm tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("lunch 12pm tmrw"), "lunch 12pm tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("call TDY"), "call today")
    }

    func testNormalizeLeavesNormalTextAlone() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("1pm tomorrow"), "1pm tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("dentist at 3"), "dentist at 3")
    }

    func testNonActionInputReturnsNil() {
        XCTAssertNil(ExplicitActionParser.parse("add lunch @ 1pm"))    // no `>`
        XCTAssertNil(ExplicitActionParser.parse(">bogus something @ 5pm")) // unknown verb
        XCTAssertNil(ExplicitActionParser.parse(">add"))               // no spec
    }
}
