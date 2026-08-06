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

    func testParsesRemindWithSeparator() {
        let call = ExplicitActionParser.parse(">remind call mom @ 5pm")
        XCTAssertEqual(call?.toolID, "reminder.add")
        XCTAssertEqual(call?.params["title"], .string("call mom"))
        XCTAssertEqual(call?.params["when"], .string("5pm"))
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
