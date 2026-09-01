// Resolved cells to CSS grid placement.
//
// Its own module, with no imports, so it can be tested directly: superactions.js
// reaches for Tauri and the DOM the moment it loads, and this is the arithmetic
// that can silently move a tile. The macOS shell keeps the same split for the
// same reason (LaunchpadGrid.swift).
//
// The core has already decided which cell every tile occupies; nothing here
// works out an arrangement.

/**
 * The shape the drawing declared, or how far the tiles reach without one.
 *
 * Deriving is the fallback because it cannot see a trailing empty track: a
 * six-wide drawing whose last column is all "." reaches five, and every tile
 * renders a sixth too wide. Mirrored in LaunchpadGrid.swift.
 */
export function gridShape(tiles, declared) {
    if (declared?.columns && declared?.rows) {
        return { columns: declared.columns, rows: declared.rows };
    }
    if (!tiles || tiles.length === 0) return { columns: 1, rows: 1 };
    return {
        columns: Math.max(1, ...tiles.map((t) => t.col + t.col_span)),
        rows: Math.max(1, ...tiles.map((t) => t.row + t.row_span)),
    };
}

/**
 * One tile's `grid-column` / `grid-row`.
 *
 * CSS grid lines are 1-based and the core counts cells from 0, which is the
 * whole reason this is a named function with a test rather than two template
 * literals inline.
 */
export function gridPlacement(tile) {
    return {
        column: `${tile.col + 1} / span ${tile.col_span}`,
        row: `${tile.row + 1} / span ${tile.row_span}`,
    };
}
