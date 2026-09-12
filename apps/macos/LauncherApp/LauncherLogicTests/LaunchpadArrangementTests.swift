import XCTest
@testable import LauncherLogic

/// Moving tiles by dragging, as arithmetic on cells.
///
/// A drop is the one moment a gesture becomes an edit to the user's file, so
/// what it may do is pinned here rather than discovered on screen. The default
/// strip comes from the shared fixture; the smaller drawings are built by hand
/// where a gap or a wide tile is the point.
final class LaunchpadArrangementTests: XCTestCase {
    private let sixByThree = LaunchpadGrid.Shape(columns: 6, rows: 3)

    /// The default layout exactly as the core ships it.
    private func defaultTiles() -> [LaunchpadTileModel] {
        try! LaunchpadFixture.layout().tiles
    }

    /// A tile with only what placement reads: its id and its cells.
    private func tile(_ id: String, col: Int, row: Int, colSpan: Int = 1, rowSpan: Int = 1) -> LaunchpadTileModel {
        LaunchpadTileModel(
            actionId: id, title: id, size: .s, role: .toggle, mnemonic: nil,
            col: col, row: row, colSpan: colSpan, rowSpan: rowSpan
        )
    }

    private func corner(_ tiles: [LaunchpadTileModel], _ id: String) -> LaunchpadArrangement.Cell? {
        tiles.first { $0.actionId == id }.map { LaunchpadArrangement.Cell(col: $0.col, row: $0.row) }
    }

    private func ids(_ tiles: [LaunchpadTileModel]) -> [String] { tiles.map(\.actionId) }

    // MARK: Trading places

    func testTwoSmallTilesTradePlaces() throws {
        let tiles = defaultTiles()
        let swapped = try XCTUnwrap(
            LaunchpadArrangement.swapping("bluetooth", with: "mic", in: tiles, shape: sixByThree))

        XCTAssertEqual(corner(swapped, "bluetooth"), corner(tiles, "mic"))
        XCTAssertEqual(corner(swapped, "mic"), corner(tiles, "bluetooth"))
        XCTAssertEqual(swapped.count, tiles.count, "nothing is lost or invented")
    }

    func testATradeLeavesTheRestWhereTheyWere() throws {
        let tiles = defaultTiles()
        let swapped = try XCTUnwrap(
            LaunchpadArrangement.swapping("wifi", with: "shutdown", in: tiles, shape: sixByThree))

        for stayed in tiles where !["wifi", "shutdown"].contains(stayed.actionId) {
            XCTAssertEqual(corner(swapped, stayed.actionId), corner(tiles, stayed.actionId), stayed.actionId)
        }
    }

    func testTheResultIsInReadingOrder() throws {
        let tiles = defaultTiles()
        let swapped = try XCTUnwrap(
            LaunchpadArrangement.swapping("bluetooth", with: "mic", in: tiles, shape: sixByThree))

        // Mic now sits where Bluetooth was, at column 2 of the top row, so it
        // comes right after the L slot; Bluetooth is first on the bottom row.
        let expected = ["lslot", "mic", "wifi", "battery", "weather", "theme", "keepawake",
                        "screensaver", "bluetooth", "restart", "shutdown", "nowplaying"]
        XCTAssertEqual(ids(swapped), expected)
    }

    func testASmallTileCannotTradeWithALargeOneWhoseNewFootprintIsTaken() {
        // The L slot moved to Bluetooth's cell would cover Wi-Fi, Theme and
        // Keep Awake.
        XCTAssertNil(
            LaunchpadArrangement.swapping("bluetooth", with: "lslot", in: defaultTiles(), shape: sixByThree))
        XCTAssertNil(
            LaunchpadArrangement.swapping("lslot", with: "bluetooth", in: defaultTiles(), shape: sixByThree))
    }

    func testALargeTileTradesWhenItsNewFootprintIsFree() throws {
        // "lslot lslot mic  ."
        // "lslot lslot .    ."
        let tiles = [tile("lslot", col: 0, row: 0, colSpan: 2, rowSpan: 2), tile("mic", col: 2, row: 0)]
        let shape = LaunchpadGrid.Shape(columns: 4, rows: 2)

        let swapped = try XCTUnwrap(LaunchpadArrangement.swapping("mic", with: "lslot", in: tiles, shape: shape))

        XCTAssertEqual(corner(swapped, "lslot"), LaunchpadArrangement.Cell(col: 2, row: 0))
        XCTAssertEqual(corner(swapped, "mic"), LaunchpadArrangement.Cell(col: 0, row: 0))
    }

    func testATradeThatWouldLeaveTheGridIsRefused() throws {
        // "nowplaying nowplaying ."
        // ".          .          mic"
        // Now Playing at Mic's cell would reach a fourth column.
        let tiles = [tile("nowplaying", col: 0, row: 0, colSpan: 2), tile("mic", col: 2, row: 1)]
        let shape = LaunchpadGrid.Shape(columns: 3, rows: 2)
        XCTAssertNil(LaunchpadArrangement.swapping("mic", with: "nowplaying", in: tiles, shape: shape))

        // One column to the left, it fits.
        let roomy = [tile("nowplaying", col: 0, row: 0, colSpan: 2), tile("mic", col: 1, row: 1)]
        let swapped = try XCTUnwrap(
            LaunchpadArrangement.swapping("mic", with: "nowplaying", in: roomy, shape: shape))
        XCTAssertEqual(corner(swapped, "nowplaying"), LaunchpadArrangement.Cell(col: 1, row: 1))
        XCTAssertEqual(corner(swapped, "mic"), LaunchpadArrangement.Cell(col: 0, row: 0))
    }

    func testATileDroppedOnItselfIsNotATrade() {
        XCTAssertNil(LaunchpadArrangement.swapping("mic", with: "mic", in: defaultTiles(), shape: sixByThree))
    }

    func testAnUnknownTileIsRefused() {
        XCTAssertNil(LaunchpadArrangement.swapping("mic", with: "ghost", in: defaultTiles(), shape: sixByThree))
        XCTAssertNil(LaunchpadArrangement.swapping("ghost", with: "mic", in: defaultTiles(), shape: sixByThree))
        XCTAssertNil(
            LaunchpadArrangement.moving("ghost", to: .init(col: 0, row: 0), in: defaultTiles(), shape: sixByThree))
    }

    // MARK: Moving into a gap

    func testATileMovesIntoAGapThatFits() throws {
        // "mic . ."
        let tiles = [tile("mic", col: 0, row: 0)]
        let moved = try XCTUnwrap(
            LaunchpadArrangement.moving("mic", to: .init(col: 2, row: 0), in: tiles, shape: LaunchpadGrid.Shape(columns: 3, rows: 1)))

        XCTAssertEqual(corner(moved, "mic"), LaunchpadArrangement.Cell(col: 2, row: 0))
    }

    func testAMoveOntoAnotherTileIsRefused() {
        // Restart's cell.
        XCTAssertNil(
            LaunchpadArrangement.moving("mic", to: .init(col: 1, row: 2), in: defaultTiles(), shape: sixByThree))
    }

    func testAMovePastTheEdgeIsRefused() {
        // Now Playing is three wide; from column 4 it would reach column 6.
        XCTAssertNil(
            LaunchpadArrangement.moving("nowplaying", to: .init(col: 4, row: 2), in: defaultTiles(), shape: sixByThree))
        XCTAssertNil(
            LaunchpadArrangement.moving("mic", to: .init(col: -1, row: 2), in: defaultTiles(), shape: sixByThree))
    }

    func testAWideTileShiftsOverItsOwnCells() throws {
        // "nowplaying nowplaying ." -> ". nowplaying nowplaying"
        let tiles = [tile("nowplaying", col: 0, row: 0, colSpan: 2)]
        let moved = try XCTUnwrap(
            LaunchpadArrangement.moving("nowplaying", to: .init(col: 1, row: 0), in: tiles, shape: LaunchpadGrid.Shape(columns: 3, rows: 1)))

        XCTAssertEqual(corner(moved, "nowplaying"), LaunchpadArrangement.Cell(col: 1, row: 0))
    }

    func testMovingToWhereItAlreadyIsChangesNothing() {
        let tiles = defaultTiles()
        XCTAssertEqual(
            LaunchpadArrangement.moving("mic", to: .init(col: 0, row: 2), in: tiles, shape: sixByThree), tiles)
    }

    // MARK: Writing back

    /// The keys `look_engine::launchpad::Placement` declares, spelled as the
    /// wire spells them. Renamed there and not here, the save silently reads
    /// nothing, so this pins the spelling the way the layout fixture does.
    func testASaveRequestSpellsItsKeysTheWayTheCoreReadsThem() throws {
        let tiles = [tile("nowplaying", col: 3, row: 2, colSpan: 3)]
        let data = try XCTUnwrap(LaunchpadArrangement.saveRequest(tiles, shape: sixByThree))
        let request = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(request["columns"] as? Int, 6)
        XCTAssertEqual(request["rows"] as? Int, 3)
        let placed = try XCTUnwrap((request["tiles"] as? [[String: Any]])?.first)
        XCTAssertEqual(Set(placed.keys), ["action_id", "col", "row", "col_span", "row_span"])
        XCTAssertEqual(placed["action_id"] as? String, "nowplaying")
        XCTAssertEqual(placed["col"] as? Int, 3)
        XCTAssertEqual(placed["row"] as? Int, 2)
        XCTAssertEqual(placed["col_span"] as? Int, 3)
        XCTAssertEqual(placed["row_span"] as? Int, 1)
    }

    // MARK: What is under a cell

    func testTheTileUnderACellIsFoundThroughItsWholeFootprint() {
        let tiles = defaultTiles()
        XCTAssertEqual(LaunchpadArrangement.tile(at: .init(col: 1, row: 1), in: tiles)?.actionId, "lslot")
        XCTAssertEqual(LaunchpadArrangement.tile(at: .init(col: 5, row: 1), in: tiles)?.actionId, "weather")
        XCTAssertEqual(LaunchpadArrangement.tile(at: .init(col: 5, row: 2), in: tiles)?.actionId, "nowplaying")
    }

    func testAGapHasNoTile() {
        let tiles = [tile("mic", col: 0, row: 0)]
        XCTAssertNil(LaunchpadArrangement.tile(at: .init(col: 1, row: 0), in: tiles))
    }
}
