import XCTest
@testable import LauncherLogic

/// The launchpad layout crosses the FFI as JSON and is decoded here with
/// `.convertFromSnakeCase`, so the core's `action_id` arrives as `actionId`.
///
/// Worth pinning because every way this contract breaks is silent.
/// `EngineBridge.launchpadLayout()` decodes with `try?` and returns `[]` on
/// failure, so a renamed field does not raise anything - it renders an empty
/// launchpad on the screen shown at every launch. The JS shell reads the raw
/// object (`tile.action_id`), so the two shells never even read the same
/// spelling, and a rename breaks them in two different silent ways.
///
/// The fixture is the one the Rust and JS tests read. Regenerate all three
/// together with:
///
///     UPDATE_FIXTURES=1 cargo test --manifest-path bridge/ffi/Cargo.toml
final class LaunchpadContractTests: XCTestCase {
    private func fixtureJSON() throws -> Data {
        // Up from LauncherLogicTests/ to the repo root: LauncherApp, macos,
        // apps, then the root itself.
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let fixture = root.appendingPathComponent("bridge/ffi/tests/fixtures/launchpad_layout.json")
        return try Data(contentsOf: fixture)
    }

    private func decode(_ data: Data) throws -> [LaunchpadTileModel] {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode([LaunchpadTileModel].self, from: data)
    }

    func testTheCoresLayoutDecodesIntoTiles() throws {
        let tiles = try decode(fixtureJSON())

        // Non-empty is the whole point: `[]` is exactly what the shipping
        // decode path produces when the contract is broken, so a test that
        // only checked "it decoded" would pass on the failure it exists for.
        XCTAssertFalse(tiles.isEmpty, "an empty layout is the failure mode, not a pass")

        for tile in tiles {
            XCTAssertFalse(tile.actionId.isEmpty, "every tile is identified by its action id")
        }
    }

    func testEveryTileArrivesWithItsResolvedCell() throws {
        let byID = Dictionary(
            try decode(fixtureJSON()).map { ($0.actionId, $0) }, uniquingKeysWith: { first, _ in first })

        func cell(_ id: String) -> [Int]? {
            byID[id].map { [$0.col, $0.row, $0.columnSpan, $0.rowSpanCount] }
        }

        // The three tiles that are not a plain 1x1, so between them they cover
        // every way the geometry can arrive wrong: a wide tile, a tall one, and
        // one that is both. `col_span` -> `colSpan` is the snake-case
        // conversion, and a slip there decodes to nothing rather than to a
        // wrong number, so these would fail loudly.
        XCTAssertEqual(cell(LaunchpadActionID.lSlot), [0, 0, 2, 2])
        XCTAssertEqual(cell(LaunchpadActionID.weather), [5, 0, 1, 2])
        XCTAssertEqual(cell(LaunchpadActionID.nowPlaying), [3, 2, 3, 1])
    }

    func testTheDecodedGridTilesWithNoGapOrOverlap() throws {
        let tiles = try decode(fixtureJSON())
        var owner: [String: String] = [:]

        for tile in tiles {
            for row in tile.row..<(tile.row + tile.rowSpanCount) {
                for col in tile.col..<(tile.col + tile.columnSpan) {
                    let key = "\(col),\(row)"
                    XCTAssertNil(owner[key], "\(tile.actionId) overlaps \(owner[key] ?? "") at \(key)")
                    owner[key] = tile.actionId
                }
            }
        }

        // The core asserts this too. Repeating it on the decoded side is not
        // redundant: it is the half that proves the numbers SURVIVED the trip,
        // rather than that they were right when they left.
        XCTAssertEqual(owner.count, 18, "the default layout covers 6x3 exactly once")
    }

    func testMnemonicsAndLabelsSurviveTheSnakeCaseConversion() throws {
        let byID = Dictionary(
            try decode(fixtureJSON()).map { ($0.actionId, $0) }, uniquingKeysWith: { first, _ in first })

        // `mnemonic` decodes from a one-character string, and Cmd+<char> is
        // dead rather than wrong when it arrives nil.
        XCTAssertEqual(byID[LaunchpadActionID.shutdown]?.mnemonic, "D")
        XCTAssertEqual(byID[LaunchpadActionID.bluetooth]?.mnemonic, "B")

        // `on_label` -> `onLabel` is the conversion itself. A toggle that loses
        // these still renders, captioned with a generic On/Off, which is the
        // kind of wrong nobody files a bug about.
        XCTAssertNotNil(byID[LaunchpadActionID.theme]?.onLabel)
        XCTAssertNotNil(byID[LaunchpadActionID.theme]?.offLabel)
    }
}
