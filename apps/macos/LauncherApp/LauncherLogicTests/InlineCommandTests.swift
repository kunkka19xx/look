import XCTest

@testable import LauncherLogic

final class InlineCommandTests: XCTestCase {
    func testExtractSupportsBothSlashAndColonPrefixes() {
        let commands = [
            ("calc", "calc 2+2", "2+2"),
            ("todo", "todo groceries", "groceries"),
            ("pomo", "pomo 25", "25"),
            ("kill", "kill chrome", "chrome"),
            ("speed", "speed", ""),
            ("sys", "sys", ""),
            ("shell", "shell ls -la", "ls -la"),
        ]

        for (id, inputSuffix, expectedArgs) in commands {
            // Test with slash `/`
            let slashInput = "/\(inputSuffix)"
            let slashMatch = InlineCommand.extract(from: slashInput)
            XCTAssertNotNil(slashMatch, "Expected match for slash input: \(slashInput)")
            XCTAssertEqual(slashMatch?.id, id)
            XCTAssertEqual(slashMatch?.args, expectedArgs)
            XCTAssertEqual(slashMatch?.hasSpace, !expectedArgs.isEmpty)

            // Test with colon `:`
            let colonInput = ":\(inputSuffix)"
            let colonMatch = InlineCommand.extract(from: colonInput)
            XCTAssertNotNil(colonMatch, "Expected match for colon input: \(colonInput)")
            XCTAssertEqual(colonMatch?.id, id)
            XCTAssertEqual(colonMatch?.args, expectedArgs)
            XCTAssertEqual(colonMatch?.hasSpace, !expectedArgs.isEmpty)
        }
    }

    func testExtractRejectsNonCommandsAndPaths() {
        let nonCommands = [
            "/Users/dam/Documents",
            "/Applications/Look.app",
            ":Users/dam/Documents",
            "/notacommand",
            ":notacommand",
            "calc 2+2",
            "regular search query",
            "",
            "   ",
        ]

        for query in nonCommands {
            XCTAssertNil(InlineCommand.extract(from: query), "Should not extract command from: \(query)")
        }
    }

    func testIsDiscoveryQuery() {
        // Bare prefix opens menu
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: "/"))
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: ":"))

        // Prefix + command prefix or match filters menu
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: "/c"))
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: ":c"))
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: "/calc"))
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: ":calc"))
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: "/todo"))
        XCTAssertTrue(InlineCommand.isDiscoveryQuery(input: ":todo"))

        // Live triggers with space are NOT discovery queries (they transition immediately)
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: "/calc 2+2"))
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: ":calc 2+2"))
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: "/kill chrome"))
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: ":kill chrome"))

        // Paths and queries without command matches should NOT trigger discovery menu
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: "/Users/dam"))
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: "/System/Library"))
        XCTAssertFalse(InlineCommand.isDiscoveryQuery(input: "hello world"))
    }

    func testFilterExtraction() {
        XCTAssertEqual(InlineCommand.filter(from: "/calc"), "calc")
        XCTAssertEqual(InlineCommand.filter(from: ":calc"), "calc")
        XCTAssertEqual(InlineCommand.filter(from: "/"), "")
        XCTAssertEqual(InlineCommand.filter(from: ":"), "")
        XCTAssertEqual(InlineCommand.filter(from: "  /pomo  "), "pomo")
        XCTAssertEqual(InlineCommand.filter(from: "regular"), "")
    }
}
