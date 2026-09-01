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
    private func fixtureTiles() throws -> [LaunchpadTileModel] {
        try LaunchpadFixture.layout().tiles
    }

    /// The payload as a lib built before the shape joined it sends: the tile
    /// array on its own.
    private func bareTileArray() throws -> Data {
        let object = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: LaunchpadFixture.json()) as? [String: Any])
        return try JSONSerialization.data(withJSONObject: XCTUnwrap(object["tiles"]))
    }

    func testTheCoresLayoutDecodesIntoTiles() throws {
        let tiles = try fixtureTiles()

        // Non-empty is the whole point: `[]` is exactly what the shipping
        // decode path produces when the contract is broken, so a test that
        // only checked "it decoded" would pass on the failure it exists for.
        XCTAssertFalse(tiles.isEmpty, "an empty layout is the failure mode, not a pass")

        for tile in tiles {
            XCTAssertFalse(tile.actionId.isEmpty, "every tile is identified by its action id")
        }
    }

    func testTheLayoutArrivesWithTheShapeTheDrawingDeclared() throws {
        // Derived from the tiles this would also be 6x3, which is the point: a
        // drawing whose last column is empty only differs from its own extent,
        // so the declared shape has to be what the payload carries.
        XCTAssertEqual(try LaunchpadFixture.layout().shape, LaunchpadGrid.Shape(columns: 6, rows: 3))
    }

    func testABareTileArrayStillDecodes() throws {
        // An app bundle linked against an older liblook_ffi.a sends this. It
        // has to keep rendering, with the grid deriving its own shape.
        let layout = try LaunchpadFixture.decode(LaunchpadLayout.self, from: bareTileArray())

        XCTAssertFalse(layout.tiles.isEmpty, "the tiles still decode without the wrapper")
        XCTAssertNil(layout.shape, "there is no declared shape to read")
    }

    func testTilesWithoutResolvedCellsDecodeToNothing() throws {
        // A lib old enough to send a bare array may predate the coordinates
        // too. The decode throws, `EngineBridge` turns that into `.empty`, and
        // the launchpad stays off rather than placing tiles on nothing - the
        // rule `normalizeLayout` applies on the linows side.
        let json = #"[{"action_id":"mic","title":"Mic","size":"s","role":"toggle"}]"#
        XCTAssertThrowsError(
            try LaunchpadFixture.decode(LaunchpadLayout.self, from: Data(json.utf8)))
    }

    func testEveryTileArrivesWithItsResolvedCell() throws {
        let byID = Dictionary(
            try fixtureTiles().map { ($0.actionId, $0) }, uniquingKeysWith: { first, _ in first })

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
        let tiles = try fixtureTiles()
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
            try fixtureTiles().map { ($0.actionId, $0) }, uniquingKeysWith: { first, _ in first })

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
