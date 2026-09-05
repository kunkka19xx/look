import XCTest
@testable import LauncherLogic

/// Who owns a plain Cmd+letter chord, checked across the language boundary.
///
/// The list lives in Rust (`look_qactions::chord_owner`) because that is where
/// a launchpad tile's mnemonic is validated, but the chords it describes are
/// declared in Swift as menu `keyboardShortcut`s. AppKit dispatches those before
/// Look's own key monitor runs, so a tile holding one silently never fires -
/// and nothing in either language would notice the two lists disagreeing.
///
/// This is the guard: add a plain Cmd+letter menu item without telling the core
/// and this fails, rather than a user finding a dead key.
final class ChordOwnershipTests: XCTestCase {
    /// Every plain `Cmd+<letter>` the menus claim. Shift/Option variants do not
    /// collide with a tile mnemonic, which is `Cmd` alone.
    private func menuClaimedLetters() throws -> Set<String> {
        // Up from LauncherLogicTests/ to LauncherApp/, which holds look-app/.
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<2 { root.deleteLastPathComponent() }
        let source = try String(
            contentsOf: root.appendingPathComponent("look-app/look_appApp.swift"),
            encoding: .utf8)

        var claimed: Set<String> = []
        let pattern = #"\.keyboardShortcut\("([A-Za-z])",\s*modifiers:\s*\[([^\]]*)\]"#
        let regex = try NSRegularExpression(pattern: pattern)
        for match in regex.matches(in: source, range: NSRange(source.startIndex..., in: source)) {
            guard let keyRange = Range(match.range(at: 1), in: source),
                  let modRange = Range(match.range(at: 2), in: source)
            else { continue }
            let modifiers = source[modRange]
            guard modifiers.contains(".command"),
                  !modifiers.contains(".shift"),
                  !modifiers.contains(".option"),
                  !modifiers.contains(".control")
            else { continue }
            claimed.insert(source[keyRange].uppercased())
        }
        return claimed
    }

    func testTheMenusClaimOnlyChordsTheCoreKnowsAreReserved() throws {
        // Kept in step by hand today; the core's list is `chord_owner`. Q is
        // Quit. Anything else added here has to be added there too, or a tile
        // may be granted a key the OS eats first.
        let knownReserved: Set<String> = ["Q"]

        XCTAssertEqual(
            try menuClaimedLetters(), knownReserved,
            "a plain Cmd+letter menu shortcut changed - look_qactions::chord_owner must match")
    }

    func testATileMnemonicNeverCollidesWithAMenuShortcut() throws {
        // The failure this exists to prevent: a tile takes a letter the menus
        // already own, and the key does nothing with no explanation.
        let claimed = try menuClaimedLetters()
        for tile in try LaunchpadFixture.layout().tiles {
            guard let mnemonic = tile.mnemonic else { continue }
            XCTAssertFalse(
                claimed.contains(String(mnemonic).uppercased()),
                "\(tile.actionId) wants Cmd+\(mnemonic), which a menu already claims")
        }
    }
}
