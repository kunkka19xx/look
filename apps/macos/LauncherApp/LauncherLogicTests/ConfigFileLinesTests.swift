import XCTest
@testable import LauncherLogic

final class ConfigFileLinesTests: XCTestCase {
    func testRepeatedSaveCyclesDoNotGrowTheFile() {
        // The regression this guards: both writers rendered with
        // `lines.joined(separator: "\n") + "\n"` over a parse that keeps the trailing
        // empty element, so every save appended one more blank line. The pomo timer
        // saves often, which is how configs reached dozens of trailing blanks.
        var text = "app_scan_depth=3\nui_font_size=14\n"

        for _ in 0..<50 {
            var lines = ConfigFileLines.parse(text)
            ConfigFileLines.upsert(&lines, key: "pomo_timer_style", value: "modern")
            text = ConfigFileLines.render(lines)
        }

        XCTAssertEqual(text, "app_scan_depth=3\nui_font_size=14\npomo_timer_style=modern\n")
    }

    func testRepairIsANoOpOnAnAlreadyCleanFile() {
        // Launch repairs the config in place, but must leave a clean file byte-identical:
        // an unconditional rewrite would bump the mtime and wake the config watcher on
        // every launch.
        let clean = """
        # look configuration

        app_scan_depth=3

        # UI theme
        ui_font_size=14

        """

        XCTAssertEqual(ConfigFileLines.render(ConfigFileLines.parse(clean)), clean)
    }

    func testRepairCleansAScarredFileInOnePass() {
        let scarred = "# UI theme\nui_font_size=14\n\n# UI theme\n\n\n# UI theme\n\n\n\n"

        let repaired = ConfigFileLines.render(ConfigFileLines.parse(scarred))

        XCTAssertEqual(repaired, "# UI theme\nui_font_size=14\n")
        XCTAssertEqual(ConfigFileLines.render(ConfigFileLines.parse(repaired)), repaired)
    }

    func testRenderEndsWithExactlyOneTrailingNewline() {
        let rendered = ConfigFileLines.render(["app_scan_depth=3", "", ""])

        XCTAssertEqual(rendered, "app_scan_depth=3\n")
    }

    func testUpsertRewritesInPlaceAndAppendsWhenAbsent() {
        var lines = ConfigFileLines.parse("# UI theme\nui_font_size=14\n")

        ConfigFileLines.upsert(&lines, key: "ui_font_size", value: "18")
        ConfigFileLines.upsert(&lines, key: "inner_gap", value: "10")

        XCTAssertEqual(ConfigFileLines.render(lines), "# UI theme\nui_font_size=18\ninner_gap=10\n")
    }

    func testRemoveDropsOnlyTheNamedKey() {
        var lines = ConfigFileLines.parse("pomo_music_folder=/tmp\npomo_timer_style=modern\n")

        ConfigFileLines.remove(&lines, key: "pomo_music_folder")

        XCTAssertEqual(ConfigFileLines.render(lines), "pomo_timer_style=modern\n")
    }

    func testKeyValuesIgnoresCommentsAndBlanks() {
        let values = ConfigFileLines.keyValues("# a comment\n\nui_font_size=14  # trailing\napp_scan_depth=3\n")

        XCTAssertEqual(values["ui_font_size"], "14")
        XCTAssertEqual(values["app_scan_depth"], "3")
        XCTAssertNil(values["# a comment"])
    }

    func testDuplicateCommentsCollapseToTheFirstOccurrence() {
        // The regression this guards: a writer that could not detect its own header
        // appended one copy plus a blank line on every save.
        let normalized = ConfigFileLines.normalize([
            "# UI theme",
            "ui_font_size=14",
            "",
            "# UI theme",
            "",
            "# UI theme",
            "",
            "# UI theme",
        ])

        XCTAssertEqual(normalized, [
            "# UI theme",
            "ui_font_size=14",
        ])
    }

    func testRepeatedCommentIsDroppedEvenWhenKeysFollowIt() {
        let normalized = ConfigFileLines.normalize([
            "# UI theme",
            "ui_font_size=14",
            "",
            "# UI theme",
            "ui_border_red=0.80",
        ])

        XCTAssertEqual(normalized, [
            "# UI theme",
            "ui_font_size=14",
            "",
            "ui_border_red=0.80",
        ])
    }

    func testDistinctUserCommentsAreNeverTouched() {
        let input = [
            "# look configuration",
            "# Backend indexing",
            "app_scan_depth=3",
            "",
            "# my own note about this value",
            "file_scan_depth=6",
        ]

        XCTAssertEqual(ConfigFileLines.normalize(input), input)
    }

    func testTrailingCommentSurvivesWhenNotADuplicate() {
        let input = [
            "app_scan_depth=3",
            "",
            "# TODO revisit the scan roots",
        ]

        XCTAssertEqual(ConfigFileLines.normalize(input), input)
    }

    func testBlankRunsCollapseAndEdgeBlanksAreDropped() {
        let normalized = ConfigFileLines.normalize([
            "",
            "",
            "app_scan_depth=3",
            "",
            "",
            "",
            "ui_font_size=14",
            "",
            "",
        ])

        XCTAssertEqual(normalized, [
            "app_scan_depth=3",
            "",
            "ui_font_size=14",
        ])
    }

    func testKeyLinesAreNeverLost() {
        let scarred = [
            "app_scan_depth=3",
            "# UI theme",
            "",
            "# UI theme",
            "ui_font_size=14",
            "",
            "# Added by look update",
            "alias_note=Notion|Obsidian",
            "",
            "# UI theme",
        ]

        let normalized = ConfigFileLines.normalize(scarred)
        let keys = normalized.filter { !$0.hasPrefix("#") && !$0.isEmpty }

        XCTAssertEqual(keys, [
            "app_scan_depth=3",
            "ui_font_size=14",
            "alias_note=Notion|Obsidian",
        ])
    }

    func testNormalizeIsIdempotent() {
        let scarred = [
            "app_scan_depth=3",
            "",
            "# UI theme",
            "",
            "# UI theme",
            "",
            "",
            "# UI theme",
            "ui_font_size=14",
            "",
        ]

        let once = ConfigFileLines.normalize(scarred)

        XCTAssertEqual(once, ConfigFileLines.normalize(once))
    }

    func testConfigWithNoCommentsAtAllIsPreserved() {
        // A user who stripped every comment by hand must not have anything re-added.
        let input = [
            "app_scan_depth=3",
            "ui_font_size=14",
        ]

        XCTAssertEqual(ConfigFileLines.normalize(input), input)
    }
}
