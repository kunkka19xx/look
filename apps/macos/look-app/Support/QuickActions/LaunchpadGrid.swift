import CoreGraphics

/// Where each launchpad tile sits, in points.
///
/// Pure arithmetic, deliberately outside the view. A SwiftUI body cannot be
/// asserted on, and placement is exactly the part that can silently move a tile
/// or change the panel's height - on the one screen shown at every launch. As a
/// plain struct it is pinned against the same geometry golden the core is.
///
/// The core has already solved WHICH cell each tile occupies; this only turns
/// cells into points.
struct LaunchpadGrid {
    let columns: Int
    let rows: Int
    private let rowHeight: CGFloat
    private let gap: CGFloat

    /// The shape a drawing declared, as the core resolved it.
    nonisolated struct Shape: Decodable, Equatable {
        let columns: Int
        let rows: Int
    }

    /// The declared shape, else how far the tiles reach. Deriving is the
    /// fallback because it cannot see a trailing empty track: a six-wide drawing
    /// whose last column is all "." reaches five, and every tile renders a sixth
    /// too wide.
    init(tiles: [LaunchpadTileModel], declared: Shape? = nil, rowHeight: CGFloat, gap: CGFloat) {
        // At least 1: an empty layout must not divide by zero. The core
        // guarantees it never sends one, and this does not depend on that.
        columns = max(1, declared?.columns ?? tiles.map { $0.col + $0.columnSpan }.max() ?? 0)
        rows = max(1, declared?.rows ?? tiles.map { $0.row + $0.rowSpanCount }.max() ?? 0)
        self.rowHeight = rowHeight
        self.gap = gap
    }

    /// The launchpad's height. Was hardcoded to three rows; a drawing decides
    /// it now, within the core's cap.
    var height: CGFloat { span(rows, of: rowHeight) }

    /// One column's width at this container width. Columns share what the gaps
    /// leave behind, so a tile is never a fixed number of points wide.
    func cellWidth(total: CGFloat) -> CGFloat {
        max(0, (total - gap * CGFloat(columns - 1)) / CGFloat(columns))
    }

    /// The tile's rectangle, relative to the grid's top-left.
    func frame(for tile: LaunchpadTileModel, totalWidth: CGFloat) -> CGRect {
        let cell = cellWidth(total: totalWidth)
        return CGRect(
            x: CGFloat(tile.col) * (cell + gap),
            y: CGFloat(tile.row) * (rowHeight + gap),
            width: span(tile.columnSpan, of: cell),
            height: span(tile.rowSpanCount, of: rowHeight)
        )
    }

    /// `count` units plus the gaps between them - never after the last one,
    /// which is what makes a spanning tile line up with its neighbours.
    private func span(_ count: Int, of unit: CGFloat) -> CGFloat {
        CGFloat(count) * unit + gap * CGFloat(count - 1)
    }
}
