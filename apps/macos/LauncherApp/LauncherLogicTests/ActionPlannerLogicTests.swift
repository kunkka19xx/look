import XCTest
@testable import LauncherLogic

/// Covers the pure planner pieces: the `format` schema and the response parser.
/// The networking `ActionPlanner` needs a live model and is smoke-tested by hand.
final class ActionPlannerLogicTests: XCTestCase {
    // ── format schema ───────────────────────────────────────────────────

    func testSchemaConstrainsToolToRegisteredIDs() {
        let schema = ActionPlanSchema.chatFormat(toolIDs: ["calendar.add_event", "reminder.add"])
        guard
            let obj = schema.objectValue,
            let props = obj["properties"]?.objectValue,
            let steps = props["steps"]?.objectValue,
            let items = steps["items"]?.objectValue,
            let itemProps = items["properties"]?.objectValue,
            let tool = itemProps["tool"]?.objectValue,
            let enumVals = tool["enum"]?.arrayValue
        else {
            return XCTFail("schema shape wrong")
        }
        XCTAssertEqual(enumVals, [.string("calendar.add_event"), .string("reminder.add")])
    }

    func testSchemaParamsShape() {
        let schema = ActionPlanSchema.chatFormat(toolIDs: ["event"])
        guard
            let items = schema.objectValue?["properties"]?.objectValue?["steps"]?
                .objectValue?["items"]?.objectValue,
            let params = items["properties"]?.objectValue?["params"]?.objectValue,
            let properties = params["properties"]?.objectValue,
            let required = params["required"]?.arrayValue
        else {
            return XCTFail("schema shape wrong")
        }
        // The minimal param vocabulary the planner can emit; per-tool needs are
        // validated in tool.plan(), so nothing is schema-required.
        XCTAssertEqual(Set(properties.keys), ["title", "match", "when"])
        XCTAssertTrue(required.isEmpty)
    }

    func testSchemaSerializesToJSON() throws {
        let schema = ActionPlanSchema.chatFormat(toolIDs: ["a", "b"])
        let json = schema.jsonObject
        XCTAssertNoThrow(try JSONSerialization.data(withJSONObject: json))
    }

    // ── response parser ─────────────────────────────────────────────────

    func testParsesStepsFromChatContent() {
        let content = #"{\"steps\":[{\"tool\":\"calendar.add_event\",\"params\":{\"title\":\"Dentist\",\"when\":\"tomorrow 9am\"}}]}"#
        let data = Data(#"{"message":{"content":"\#(content)"}}"#.utf8)
        let plan = ActionPlanParser.parse(chatResponse: data)
        XCTAssertEqual(plan?.steps.count, 1)
        XCTAssertEqual(plan?.steps.first?.tool, "calendar.add_event")
        XCTAssertEqual(plan?.steps.first?.params["title"], .string("Dentist"))
        XCTAssertEqual(plan?.steps.first?.params["when"], .string("tomorrow 9am"))
    }

    func testParsesEmptyStepsAsDecline() {
        let data = Data(#"{"message":{"content":"{\"steps\":[]}"}}"#.utf8)
        let plan = ActionPlanParser.parse(chatResponse: data)
        XCTAssertEqual(plan?.steps.count, 0)
    }

    func testParseReturnsNilOnGarbage() {
        XCTAssertNil(ActionPlanParser.parse(chatResponse: Data(#"{"message":{"content":"sorry"}}"#.utf8)))
        XCTAssertNil(ActionPlanParser.parse(chatResponse: Data("{}".utf8)))
    }

    func testMessageContentExtracted() {
        let data = Data(#"{"message":{"content":"hello"}}"#.utf8)
        XCTAssertEqual(ActionPlanParser.messageContent(data), "hello")
    }
}
