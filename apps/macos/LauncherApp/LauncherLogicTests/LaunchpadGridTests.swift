import XCTest
@testable import LauncherLogic

/// Cells to points.
///
/// This exists because the launchpad renders on every launch and a screenshot
/// cannot check it: the screen is mostly live state - the L slot rotates
/// Pomo/Todo/Clock and its clock ticks, Battery carries uptime, Now Playing
/// polls, Weather updates - so two captures never compare cleanly. Placement is
/// arithmetic, so it is pinned as arithmetic.
///
/// The rectangles below are the ones the old hand-composed layout produced:
/// nested stacks whose spacing was `gap` and whose tile frames were
/// `columns * cell + gap * (columns - 1)`. Same numbers, reached from
/// coordinates instead of from nesting.
final class LaunchpadGridTests: XCTestCase {
    private let rowHeight: CGFloat = 76
    private let gap: CGFloat = 8

    /// The default layout exactly as the core ships it. Read from the shared
    /// fixture: a copy here would be a third place the layout lives.
    private func defaultTiles() -> [LaunchpadTileModel] {
        try! LaunchpadFixture.layout().tiles
    }

    /// One 1x1 tile at the origin, spelled the way the wire spells one.
    private func singleTile() -> LaunchpadTileModel {
        let json = #"{"action_id":"mic","title":"Mic","size":"s","role":"toggle","mnemonic":"M","col":0,"row":0,"col_span":1,"row_span":1,"on_label":null,"off_label":null}"#
        return try! LaunchpadFixture.decode(LaunchpadTileModel.self, from: Data(json.utf8))
    }

    private func grid(_ tiles: [LaunchpadTileModel], declared: LaunchpadGrid.Shape? = nil) -> LaunchpadGrid {
        LaunchpadGrid(tiles: tiles, declared: declared, rowHeight: rowHeight, gap: gap)
    }

    func testTheDefaultLayoutIsSixColumnsByThreeRows() {
        let grid = grid(defaultTiles())
        XCTAssertEqual(grid.columns, 6)
        XCTAssertEqual(grid.rows, 3)
        // Three rows of 76 with two 8pt gaps. The height used to be hardcoded
        // to exactly this; it is derived now, so it has to still land here.
        XCTAssertEqual(grid.height, 3 * 76 + 2 * 8)
    }

    func testADeclaredShapeKeepsATrailingEmptyTrack() {
        // `layout = ["mic . ."]`: one tile, two holes. Derived from the tile it
        // reaches one column and `mic` fills the strip; declared, it is a third
        // of it, which is what the drawing says.
        let grid = grid([singleTile()], declared: LaunchpadGrid.Shape(columns: 3, rows: 1))

        XCTAssertEqual(grid.columns, 3)
        XCTAssertEqual(grid.rows, 1)
        XCTAssertEqual(grid.cellWidth(total: 3 * 100 + 2 * gap), 100, accuracy: 0.001)
    }

    func testWithoutADeclaredShapeTheTilesStillImplyOne() {
        // The fallback for an older payload: the extent, exactly as before.
        let grid = grid(defaultTiles())
        XCTAssertEqual(grid.columns, 6)
        XCTAssertEqual(grid.rows, 3)
    }

    func testTilesTileTheWidthWithoutGapOrOverlap() {
        // The property the whole grid rests on: at any container width, a row
        // of tiles exactly spans it. Awkward widths on purpose - 800 divides
        // evenly by nothing here.
        for width in [520.0, 800.0, 1013.5] as [CGFloat] {
            let tiles = defaultTiles()
            let grid = grid(tiles)

            let bottomRow = tiles.filter { $0.row == 2 }.sorted { $0.col < $1.col }
            let boxes = bottomRow.map { grid.frame(for: $0, totalWidth: width) }

            XCTAssertEqual(boxes.first!.minX, 0, accuracy: 0.001, "row starts flush left")
            XCTAssertEqual(
                boxes.last!.maxX, width, accuracy: 0.001,
                "row ends flush right at width \(width)")

            for (left, right) in zip(boxes, boxes.dropFirst()) {
                XCTAssertEqual(
                    right.minX - left.maxX, gap, accuracy: 0.001,
                    "exactly one gap between neighbours at width \(width)")
            }
        }
    }

    func testASpanningTileCoversItsCellsAndTheGapsBetweenThem() {
        let tiles = defaultTiles()
        let grid = grid(tiles)
        let width: CGFloat = 800
        let cell = grid.cellWidth(total: width)

        // Now Playing spans 3 columns: three cells plus the two gaps inside it.
        // Getting this wrong by one gap is the classic spanning bug, and it
        // leaves a seam a user sees but cannot name.
        let nowPlaying = tiles.first { $0.actionId == "nowplaying" }!
        XCTAssertEqual(
            grid.frame(for: nowPlaying, totalWidth: width).width,
            3 * cell + 2 * gap, accuracy: 0.001)

        // Weather spans 2 rows, in one column.
        let weather = tiles.first { $0.actionId == "weather" }!
        let box = grid.frame(for: weather, totalWidth: width)
        XCTAssertEqual(box.width, cell, accuracy: 0.001)
        XCTAssertEqual(box.height, 2 * rowHeight + gap, accuracy: 0.001)
    }

    func testTheLSlotSitsAtTheOriginTwoByTwo() {
        let tiles = defaultTiles()
        let grid = grid(tiles)
        let width: CGFloat = 800
        let box = grid.frame(for: tiles.first { $0.actionId == "lslot" }!, totalWidth: width)

        XCTAssertEqual(box.minX, 0, accuracy: 0.001)
        XCTAssertEqual(box.minY, 0, accuracy: 0.001)
        XCTAssertEqual(box.width, 2 * grid.cellWidth(total: width) + gap, accuracy: 0.001)
        XCTAssertEqual(box.height, 2 * rowHeight + gap, accuracy: 0.001)
    }

    func testRowsStackByRowHeightPlusOneGap() {
        let tiles = defaultTiles()
        let grid = grid(tiles)
        let y = { (id: String) in
            grid.frame(for: tiles.first { $0.actionId == id }!, totalWidth: 800).minY
        }
        XCTAssertEqual(y("bluetooth"), 0, accuracy: 0.001)
        XCTAssertEqual(y("theme"), rowHeight + gap, accuracy: 0.001)
        XCTAssertEqual(y("mic"), 2 * (rowHeight + gap), accuracy: 0.001)
    }

    func testAShorterDrawingMakesAShorterLaunchpad() {
        // The height was hardcoded to three rows. A user who draws two must get
        // a panel that fits two, or the empty state grows a band of dead space.
        let twoRows = defaultTiles().filter { $0.row < 2 }
        XCTAssertEqual(grid(twoRows).rows, 2)
        XCTAssertEqual(grid(twoRows).height, 2 * 76 + 8)
    }

    func testAnEmptyLayoutCannotDivideByZero() {
        // The core promises it never sends one. This does not rely on that:
        // a zero-column grid would be a crash, not a blank screen.
        let grid = grid([])
        XCTAssertEqual(grid.columns, 1)
        XCTAssertEqual(grid.rows, 1)
        XCTAssertEqual(grid.cellWidth(total: 800), 800, accuracy: 0.001)
    }

    func testANarrowContainerNeverProducesANegativeCell() {
        // Five gaps at 8pt need 40pt before any tile gets a point. The launcher
        // window cannot currently get this small, but a negative width is a
        // rendering failure rather than a squeeze.
        XCTAssertEqual(grid(defaultTiles()).cellWidth(total: 10), 0, accuracy: 0.001)
    }
}
