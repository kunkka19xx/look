import XCTest
@testable import LauncherLogic

/// The monitors ask this table instead of comparing keyCodes, so a binding that
/// is missing, duplicated or stored under an unpredictable name is a shortcut
/// that silently stops working.
final class ShortcutBindingTableTests: XCTestCase {
    func testEveryShortcutIsDefinedExactlyOnce() {
        for shortcut in LocalShortcut.allCases {
            let matches = ShortcutBindingTable.all.filter { $0.shortcut == shortcut }
            XCTAssertEqual(matches.count, 1, "\(shortcut.rawValue) is defined \(matches.count) times")
        }
        XCTAssertEqual(ShortcutBindingTable.all.count, LocalShortcut.allCases.count)
    }

    /// Settings renders the recorder onto a catalog row, so a binding without
    /// one would be rebindable and invisible.
    func testEveryBindingDocumentsCatalogRows() {
        let ids = Set(ShortcutCatalog.allEntries.map(\.id))
        for definition in ShortcutBindingTable.all {
            XCTAssertTrue(ids.contains(definition.catalogID), "no catalog row for \(definition.catalogID)")
            for mirror in definition.mirrors {
                XCTAssertTrue(ids.contains(mirror), "no catalog row for mirror \(mirror)")
            }
            XCTAssertEqual(
                ShortcutCatalog.entry(definition.catalogID)?.remappable, true,
                "\(definition.catalogID) is bound but marked unremappable")
        }
    }

    /// One row, one recorder: a mirror must not also be a binding of its own.
    func testMirrorsResolveToOneBinding() {
        for definition in ShortcutBindingTable.all {
            for mirror in definition.mirrors {
                XCTAssertEqual(
                    ShortcutBindingTable.definition(catalogID: mirror)?.shortcut, definition.shortcut)
            }
        }
        XCTAssertEqual(ShortcutBindingTable.definition(catalogID: "clipboard.remove")?.shortcut, .mainTrash)
        XCTAssertNil(ShortcutBindingTable.definition(catalogID: "main.open"))
    }

    func testConfigKeysAreDerivedAndUnique() {
        XCTAssertEqual(ShortcutBindingTable.configKey(for: "main.copy"), "shortcut_main_copy")
        XCTAssertEqual(ShortcutBindingTable.configKey(for: "main.clearPicks"), "shortcut_main_clear_picks")
        XCTAssertEqual(ShortcutBindingTable.configKey(for: "view.zoomIn"), "shortcut_view_zoom_in")

        let keys = ShortcutBindingTable.all.map(\.configKey)
        XCTAssertEqual(Set(keys).count, keys.count, "two bindings share a config key")
        for key in keys {
            XCTAssertTrue(ShortcutBindingTable.isShortcutKey(key))
            XCTAssertEqual(ShortcutBindingTable.definition(configKey: key)?.configKey, key)
        }
        XCTAssertFalse(ShortcutBindingTable.isShortcutKey("launcher_hotkey"))
    }

    func testEveryBindingShipsWithADefault() {
        for definition in ShortcutBindingTable.all {
            XCTAssertFalse(definition.defaults.isEmpty, "\(definition.catalogID) has no default chord")
            for spec in definition.defaults {
                XCTAssertFalse(spec.contains("|"), "\(spec) is a list, not a chord")
                XCTAssertTrue(spec.contains("+"), "\(spec) has no modifier")
            }
        }
    }

    /// ⌘D means three documented things through one branch, and `/todo`'s ⌘Z is
    /// not the assistant's.
    func testShippedDefaultsDoNotCollide() {
        let specs = Dictionary(
            uniqueKeysWithValues: LocalShortcut.allCases.map {
                ($0, ShortcutBindingTable.definition($0).defaults)
            })
        XCTAssertEqual(ShortcutBindingTable.conflicts(specs: specs).map(\.spec), [])
    }

    func testAChordClaimedTwiceInOnePlaceIsReported() {
        var specs: [LocalShortcut: [String]] = [.mainCopy: ["cmd+c"], .mainReveal: ["cmd+c"]]
        let reported = ShortcutBindingTable.conflicts(specs: specs)
        XCTAssertEqual(reported.count, 1)
        XCTAssertEqual(reported.first?.spec, "cmd+c")
        XCTAssertEqual(reported.first?.shortcuts, [.mainCopy, .mainReveal])

        // Different places, same chord: that is how ⌘Z already works.
        specs = [.aiUndo: ["cmd+z"], .todoUndo: ["cmd+z"]]
        XCTAssertEqual(ShortcutBindingTable.conflicts(specs: specs).count, 0)

        // A menu command answers wherever Look is, so it collides with both.
        specs = [.viewZoomReset: ["cmd+z"], .todoUndo: ["cmd+z"]]
        XCTAssertEqual(ShortcutBindingTable.conflicts(specs: specs).count, 1)
    }

    /// The recorder refuses a chord SwiftUI cannot draw as a key equivalent.
    func testMenuDrivenShortcutsAreMarked() {
        XCTAssertTrue(ShortcutBindingTable.definition(.viewZoomIn).menuDriven)
        XCTAssertTrue(ShortcutBindingTable.definition(.viewSettings).menuDriven)
        XCTAssertFalse(ShortcutBindingTable.definition(.viewToggleLayout).menuDriven)
        XCTAssertFalse(ShortcutBindingTable.definition(.mainCopy).menuDriven)
    }
}
