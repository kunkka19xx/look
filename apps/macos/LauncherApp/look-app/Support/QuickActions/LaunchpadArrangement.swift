import Foundation

/// Moving tiles around the drawing by direct manipulation.
///
/// Pure arithmetic on the cells the core resolved, deliberately outside the
/// view: a drop is the one moment a gesture becomes an edit to the user's
/// file, and what it may change has to be assertable. The view turns pointer
/// positions into cells and asks these; the core then writes the drawing.
///
/// Two operations, both local. The rest of the grid never re-flows, because
/// the drawing is explicit cells and a gap in it is the user's. What a drop
/// cannot do - cover another tile, leave the grid - is refused, and the view
/// shows the refusal before the drop rather than after.
///
/// Nonisolated like the tile model: the bridge encodes a save request off the
/// main actor, and nothing here touches UI state.
nonisolated enum LaunchpadArrangement {
    /// A cell in the grid, zero-based like the wire.
    struct Cell: Equatable {
        let col: Int
        let row: Int
    }

    /// The tile covering `cell`, or nil for a gap.
    static func tile(at cell: Cell, in tiles: [LaunchpadTileModel]) -> LaunchpadTileModel? {
        tiles.first { $0.covers(cell) }
    }

    /// `tiles` with `id` and `targetID` trading top-left corners, in reading
    /// order. Nil when either is unknown, when they are one tile, or when
    /// either would leave the grid or land on a third. Two tiles of one size
    /// always trade; a small one and a large one trade only where the large
    /// one's new footprint is free.
    static func swapping(
        _ id: String,
        with targetID: String,
        in tiles: [LaunchpadTileModel],
        shape: LaunchpadGrid.Shape
    ) -> [LaunchpadTileModel]? {
        guard id != targetID,
              let held = tiles.first(where: { $0.actionId == id }),
              let target = tiles.first(where: { $0.actionId == targetID })
        else { return nil }

        let movedHeld = held.placed(atCol: target.col, row: target.row)
        let movedTarget = target.placed(atCol: held.col, row: held.row)
        let others = tiles.filter { $0.actionId != id && $0.actionId != targetID }
        guard fits(movedHeld, among: others + [movedTarget], shape: shape),
              fits(movedTarget, among: others + [movedHeld], shape: shape)
        else { return nil }
        return readingOrder(others + [movedHeld, movedTarget])
    }

    /// `tiles` with `id` moved so its top-left corner is `origin`, in reading
    /// order. Nil when the tile is unknown, would leave the grid, or would
    /// cover another tile. Its own old cells count as free, so a wide tile can
    /// shift by one column. Moving to where it already is returns `tiles`.
    static func moving(
        _ id: String,
        to origin: Cell,
        in tiles: [LaunchpadTileModel],
        shape: LaunchpadGrid.Shape
    ) -> [LaunchpadTileModel]? {
        guard let held = tiles.first(where: { $0.actionId == id }) else { return nil }
        if held.col == origin.col, held.row == origin.row { return tiles }

        let moved = held.placed(atCol: origin.col, row: origin.row)
        let others = tiles.filter { $0.actionId != id }
        guard fits(moved, among: others, shape: shape) else { return nil }
        return readingOrder(others + [moved])
    }

    /// Inside the grid and clear of every tile in `others`.
    private static func fits(
        _ tile: LaunchpadTileModel,
        among others: [LaunchpadTileModel],
        shape: LaunchpadGrid.Shape
    ) -> Bool {
        guard tile.col >= 0, tile.row >= 0,
              tile.col + tile.columnSpan <= shape.columns,
              tile.row + tile.rowSpanCount <= shape.rows
        else { return false }
        return !others.contains { $0.overlaps(tile) }
    }

    /// The order the core sends: top-left to bottom-right, which is also the
    /// order the entrance cascade plays in.
    private static func readingOrder(_ tiles: [LaunchpadTileModel]) -> [LaunchpadTileModel] {
        tiles.sorted { $0.row != $1.row ? $0.row < $1.row : $0.col < $1.col }
    }

    // MARK: Writing back

    /// The arrangement as the core's save call reads it: `{columns, rows,
    /// tiles: [{action_id, col, row, col_span, row_span}]}`, the same numbers
    /// the layout call handed out, moved. Snake case on the wire, like every
    /// other launchpad payload, so `look_engine::launchpad::Placement` decodes
    /// it by the names it declares. Nil only if encoding fails, which it
    /// cannot for these fields.
    static func saveRequest(_ tiles: [LaunchpadTileModel], shape: LaunchpadGrid.Shape) -> Data? {
        struct Placement: Encodable {
            let actionId: String
            let col: Int
            let row: Int
            let colSpan: Int
            let rowSpan: Int
        }
        struct Request: Encodable {
            let columns: Int
            let rows: Int
            let tiles: [Placement]
        }
        let request = Request(
            columns: shape.columns,
            rows: shape.rows,
            tiles: tiles.map {
                Placement(
                    actionId: $0.actionId, col: $0.col, row: $0.row,
                    colSpan: $0.columnSpan, rowSpan: $0.rowSpanCount)
            }
        )
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        return try? encoder.encode(request)
    }
}

// Nonisolated with the model it extends; an extension does not inherit that
// from the declaration.
nonisolated extension LaunchpadTileModel {
    /// Whether the tile's footprint includes `cell`.
    func covers(_ cell: LaunchpadArrangement.Cell) -> Bool {
        (col..<(col + columnSpan)).contains(cell.col)
            && (row..<(row + rowSpanCount)).contains(cell.row)
    }

    /// Whether the two footprints share a cell.
    func overlaps(_ other: LaunchpadTileModel) -> Bool {
        col < other.col + other.columnSpan && other.col < col + columnSpan
            && row < other.row + other.rowSpanCount && other.row < row + rowSpanCount
    }
}
