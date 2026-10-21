import XCTest
@testable import LauncherLogic

/// The catalog is the single source both the help screen and Settings render,
/// so the drift these tests guard against used to be shipped: two hand-written
/// tables that disagreed with each other and with the code.
final class ShortcutCatalogTests: XCTestCase {
    /// The bug this replaces: Settings listed `Cmd+1..6` naming /kill fourth,
    /// after /speed had been inserted ahead of it, so every mapping from ⌘4 on
    /// was off by one. Deriving from the catalog makes reordering enough.
    func testCommandSwitchRowMatchesTheCommandCatalog() throws {
        let commands = AppConstants.Launcher.commandCatalog
        let row = try XCTUnwrap(entry("command.switchByIndex"))

        XCTAssertEqual(row.keys, "Cmd+1..\(commands.count)")
        for command in commands {
            XCTAssertTrue(
                row.action.contains("/\(command.id)"),
                "the ⌘N row does not mention /\(command.id)")
        }
    }

    /// `/speed` sits fourth, which is exactly the position the stale table got
    /// wrong. Naming it explicitly keeps the regression legible.
    func testSpeedIsListedFourth() {
        let ids = AppConstants.Launcher.commandCatalog.map(\.id)
        XCTAssertEqual(ids.count, 7)
        XCTAssertEqual(ids[3], AppConstants.Launcher.Command.speed)
    }

    func testIDsAreUnique() {
        let ids = ShortcutCatalog.allEntries.map(\.id)
        XCTAssertEqual(Set(ids).count, ids.count, "duplicate shortcut id")
    }

    /// Ids are the handle a future user remapping binds to, so an entry without
    /// one cannot be overridden and a blank key reads as a broken row.
    func testEveryEntryIsRenderableAndAddressable() {
        for entry in ShortcutCatalog.allEntries {
            XCTAssertFalse(entry.id.isEmpty)
            XCTAssertFalse(entry.keys.isEmpty, "\(entry.id) has no keys")
            XCTAssertFalse(entry.action.isEmpty, "\(entry.id) has no description")
        }
    }

    /// Settings renders every group; the help screen renders them by topic. If a
    /// group belonged to no topic it would be visible in one surface only, which
    /// is the failure mode the consolidation removed.
    func testTopicsPartitionTheCatalog() {
        let byTopic = ShortcutTopic.allCases.flatMap { ShortcutCatalog.groups(for: $0) }
        XCTAssertEqual(byTopic.count, ShortcutCatalog.groups.count)
        for topic in ShortcutTopic.allCases {
            XCTAssertFalse(ShortcutCatalog.groups(for: topic).isEmpty, "\(topic.label) has no groups")
        }
    }

    func testGroupTitlesAreUnique() {
        let titles = ShortcutCatalog.groups.map(\.title)
        XCTAssertEqual(Set(titles).count, titles.count)
    }

    /// Prefixes are derived, so the count follows the canonical list rather than
    /// a copy of it.
    func testPrefixesComeFromTheCanonicalList() {
        let group = ShortcutCatalog.groups.first { $0.topic == .prefixes }
        XCTAssertEqual(group?.entries.count, AppConstants.Launcher.PrefixSuggestion.all.count)
    }

    /// The AI keys existed only on the help screen before this; Settings had a
    /// section for zoom but none for the assistant.
    func testAIShortcutsAreInTheCatalog() {
        let ai = ShortcutCatalog.groups(for: .ai).flatMap(\.entries)
        XCTAssertFalse(ai.isEmpty)
        XCTAssertTrue(ai.contains { $0.keys.contains("Option+Up") })
    }

    /// A typed prefix or a positional ⌘N has no chord to reassign, so a remap UI
    /// must be able to tell them apart from real bindings.
    func testUnassignableEntriesAreMarked() {
        XCTAssertEqual(entry("command.switchByIndex")?.remappable, false)
        XCTAssertEqual(entry("main.copy")?.remappable, true)
        for entry in ShortcutCatalog.groups(for: .prefixes).flatMap(\.entries) {
            XCTAssertFalse(entry.remappable, "\(entry.id) is typed text, not a chord")
        }
    }

    private func entry(_ id: String) -> ShortcutEntry? {
        ShortcutCatalog.allEntries.first { $0.id == id }
    }
}
