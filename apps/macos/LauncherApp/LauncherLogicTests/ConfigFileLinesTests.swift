import XCTest
@testable import LauncherLogic

final class ConfigFileLinesTests: XCTestCase {
    // ── A write touches only the keys it was asked to touch ─────────────

    func testWritingAKeyLeavesEveryOtherLineUntouched() {
        let original = """
        # look configuration

        ##########
        # indexing
        ##########
        app_scan_depth=3


        # UI theme
        ui_font_size=14

        """

        var lines = ConfigFileLines.parse(original)
        ConfigFileLines.upsert(&lines, key: "ui_font_size", value: "18")

        // Every comment, blank run, and divider survives exactly as written. Only the
        // one requested value differs.
        XCTAssertEqual(ConfigFileLines.render(lines), original.replacingOccurrences(
            of: "ui_font_size=14",
            with: "ui_font_size=18"
        ))
    }

    func testAddingAKeyAppendsItAndChangesNothingElse() {
        let original = "# look configuration\n\napp_scan_depth=3\n"

        var lines = ConfigFileLines.parse(original)
        ConfigFileLines.upsert(&lines, key: "inner_gap", value: "10")

        XCTAssertEqual(ConfigFileLines.render(lines), "# look configuration\n\napp_scan_depth=3\ninner_gap=10\n")
    }

    func testRemovingAKeyDropsOnlyThatLine() {
        let original = "# UI theme\nui_background_image=/tmp/a.jpg\nui_font_size=14\n"

        var lines = ConfigFileLines.parse(original)
        ConfigFileLines.remove(&lines, key: "ui_background_image")

        XCTAssertEqual(ConfigFileLines.render(lines), "# UI theme\nui_font_size=14\n")
    }

    func testRepeatedSaveCyclesDoNotGrowTheFile() {
        // The regression this guards: both writers rendered with
        // `lines.joined(separator: "\n") + "\n"` over a parse that kept the trailing
        // empty element, so every save appended one more blank line. The pomo timer
        // saves often, which is how configs reached dozens of trailing blanks.
        var text = "app_scan_depth=3\n\n# UI theme\nui_font_size=14\n"
        let expected = "app_scan_depth=3\n\n# UI theme\nui_font_size=14\npomo_timer_style=modern\n"

        for _ in 0..<50 {
            var lines = ConfigFileLines.parse(text)
            ConfigFileLines.upsert(&lines, key: "pomo_timer_style", value: "modern")
            text = ConfigFileLines.render(lines)
        }

        XCTAssertEqual(text, expected)
    }

    // ── Legacy damage repair, and its refusal to touch anything else ────

    func testRepairRemovesSurplusLegacyHeadersAndTheirBlankRuns() {
        let scarred = "# UI theme\nui_font_size=14\n\n# UI theme\n\n\n# UI theme\ninner_gap=10\n\n\n"

        let repaired = ConfigFileLines.repairingLegacyDamage(scarred)

        // The run of surplus headers and blanks collapses to a single blank separator.
        // Trailing blanks go entirely.
        XCTAssertEqual(repaired, "# UI theme\nui_font_size=14\n\ninner_gap=10\n")
    }

    func testRepairIsIdempotent() {
        let scarred = "# UI theme\nui_font_size=14\n\n# UI theme\n\n# UI theme\n"

        guard let once = ConfigFileLines.repairingLegacyDamage(scarred) else {
            return XCTFail("damaged config should have been repaired")
        }

        XCTAssertNil(ConfigFileLines.repairingLegacyDamage(once), "a repaired config must not be rewritten again")
    }

    func testRepairLeavesAnUndamagedConfigAlone() {
        // No signature, so no rewrite: not even a reformat. Returning nil is what keeps
        // launch from bumping the mtime and waking the config watcher.
        let clean = "# look configuration\n\napp_scan_depth=3\n\n# UI theme\nui_font_size=14\n"

        XCTAssertNil(ConfigFileLines.repairingLegacyDamage(clean))
    }

    func testRepairDoesNotReformatAHandWrittenConfig() {
        // Repeated `####` dividers and double blank lines are a layout the user chose.
        // The file carries no damage signature, so it must come back untouched.
        let handWritten = """
        ##########
        # indexing
        ##########
        app_scan_depth=3


        ##########
        # theme
        ##########
        ui_font_size=14

        """

        XCTAssertNil(ConfigFileLines.repairingLegacyDamage(handWritten))
    }

    func testRepairKeepsRepeatedMigrationMarkers() {
        // Two `# Added by look update` blocks are legitimate: each came from a separate
        // migration. Only the legacy header is surplus.
        let scarred = """
        # Added by look update
        app_exclude_paths=

        # UI theme

        # UI theme
        # Added by look update
        file_scan_extra_roots=

        """

        let repaired = ConfigFileLines.repairingLegacyDamage(scarred)

        XCTAssertEqual(repaired, """
        # Added by look update
        app_exclude_paths=

        # UI theme

        # Added by look update
        file_scan_extra_roots=

        """)
    }

    func testRepairNeverLosesAKey() {
        let scarred = "app_scan_depth=3\n# UI theme\n\n# UI theme\nui_font_size=14\n\n# UI theme\ninner_gap=10\n"

        guard let repaired = ConfigFileLines.repairingLegacyDamage(scarred) else {
            return XCTFail("damaged config should have been repaired")
        }

        let values = ConfigFileLines.keyValues(repaired)
        XCTAssertEqual(values["app_scan_depth"], "3")
        XCTAssertEqual(values["ui_font_size"], "14")
        XCTAssertEqual(values["inner_gap"], "10")
    }

    // ── Parsing ────────────────────────────────────────────────────────

    func testParseDropsTheTerminatingNewlineSoAppendsDoNotSitBehindABlank() {
        var lines = ConfigFileLines.parse("app_scan_depth=3\n")
        ConfigFileLines.upsert(&lines, key: "inner_gap", value: "10")

        XCTAssertEqual(ConfigFileLines.render(lines), "app_scan_depth=3\ninner_gap=10\n")
    }

    func testStripCommentKeepsHashInsideAValue() {
        // A `#` in a path is part of the value. Cutting at the first `#` anywhere would
        // truncate the background image path on read.
        XCTAssertEqual(
            ConfigFileLines.stripComment("ui_background_image=/Users/me/pic#1.png"),
            "ui_background_image=/Users/me/pic#1.png"
        )
        XCTAssertEqual(ConfigFileLines.stripComment("ui_font_size=14  # note"), "ui_font_size=14  ")
        XCTAssertEqual(ConfigFileLines.stripComment("# UI theme"), "")
    }

    func testKeyValuesRoundTripsAValueContainingHash() {
        let values = ConfigFileLines.keyValues("ui_background_image=/Users/me/pic#1.png\n")

        XCTAssertEqual(values["ui_background_image"], "/Users/me/pic#1.png")
    }

    func testUpsertMatchesAKeyWhoseValueContainsHash() {
        var lines = ConfigFileLines.parse("ui_background_image=/Users/me/pic#1.png\n")
        ConfigFileLines.upsert(&lines, key: "ui_background_image", value: "/Users/me/other.png")

        XCTAssertEqual(ConfigFileLines.render(lines), "ui_background_image=/Users/me/other.png\n")
    }

    func testKeyValuesIgnoresCommentsAndBlanks() {
        let values = ConfigFileLines.keyValues("# a comment\n\nui_font_size=14  # trailing\napp_scan_depth=3\n")

        XCTAssertEqual(values["ui_font_size"], "14")
        XCTAssertEqual(values["app_scan_depth"], "3")
        XCTAssertNil(values["# a comment"])
    }
}
