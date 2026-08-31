import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { gridPlacement, gridShape } from '../src/js/components/launchpad-grid.js';

// The golden geometry of the empty-state launchpad.
//
// It was captured from `grid-template-areas` in superactions.css, back when the
// CSS was where placement lived. It is not any more - the core resolves every
// cell and this shell only draws - so the same golden now pins the two things
// that replaced it: what the core sends, and what this shell turns that into.
//
// Keeping the file unchanged across that swap is the point. It is the same
// twelve rectangles the CSS drew, so "the layout did not move" is checkable
// rather than asserted.

const root = new URL('../../../', import.meta.url);
const golden = JSON.parse(
    readFileSync(fileURLToPath(new URL('bridge/ffi/tests/fixtures/launchpad_geometry.json', root)), 'utf8'),
);
const layout = JSON.parse(
    readFileSync(fileURLToPath(new URL('bridge/ffi/tests/fixtures/launchpad_layout.json', root)), 'utf8'),
);

test('the core places every tile where the golden says', () => {
    const fromCore = Object.fromEntries(
        layout.map((t) => [
            t.action_id,
            { col: t.col, row: t.row, col_span: t.col_span, row_span: t.row_span },
        ]),
    );
    assert.deepEqual(fromCore, golden.tiles);
});

test('the golden tiles the grid with no gap and no overlap', () => {
    // The property the shipped layout has and a user-declared one need not:
    // worth stating explicitly, because it is what silently stops being true
    // once the drawing is editable.
    const seen = new Map();
    for (const [id, t] of Object.entries(golden.tiles)) {
        for (let r = t.row; r < t.row + t.row_span; r++) {
            for (let c = t.col; c < t.col + t.col_span; c++) {
                assert.ok(c < golden.columns && r < golden.rows, `${id} runs outside the grid`);
                const key = `${c},${r}`;
                assert.ok(!seen.has(key), `${id} overlaps ${seen.get(key)} at ${key}`);
                seen.set(key, id);
            }
        }
    }
    assert.equal(seen.size, golden.columns * golden.rows, 'every cell is covered');
});

test('the shell derives the grid the drawing actually reaches', () => {
    assert.deepEqual(gridShape(layout), { columns: golden.columns, rows: golden.rows });
});

test('an empty layout cannot produce a zero-track grid', () => {
    // The core promises it never sends one. `repeat(0, 1fr)` is an invalid
    // template, which would drop every tile, so this does not rely on that.
    assert.deepEqual(gridShape([]), { columns: 1, rows: 1 });
    assert.deepEqual(gridShape(undefined), { columns: 1, rows: 1 });
});

test('placement converts the core cells onto 1-based grid lines', () => {
    // The off-by-one that would move every tile up and left by one cell, and
    // the span arithmetic that would leave a seam under Now Playing.
    const at = (id) => gridPlacement(layout.find((t) => t.action_id === id));

    assert.deepEqual(at('lslot'), { column: '1 / span 2', row: '1 / span 2' });
    assert.deepEqual(at('bluetooth'), { column: '3 / span 1', row: '1 / span 1' });
    assert.deepEqual(at('weather'), { column: '6 / span 1', row: '1 / span 2' });
    assert.deepEqual(at('nowplaying'), { column: '4 / span 3', row: '3 / span 1' });
});

test('every tile in the layout gets a placement inside the grid', () => {
    const { columns, rows } = gridShape(layout);
    for (const tile of layout) {
        const at = gridPlacement(tile);
        // "<line> / span <n>"
        const [col, , , colSpan] = at.column.split(' ');
        const [row, , , rowSpan] = at.row.split(' ');
        assert.ok(Number(col) >= 1 && Number(col) + Number(colSpan) - 1 <= columns, tile.action_id);
        assert.ok(Number(row) >= 1 && Number(row) + Number(rowSpan) - 1 <= rows, tile.action_id);
    }
});

test('the stylesheet no longer holds a second copy of the layout', () => {
    // The change this file exists to protect: placement used to be declared
    // here AND in the core, and the two had to agree. If either comes back,
    // the drawing in launchpad.toml is being silently overruled for that tile.
    const css = readFileSync(
        fileURLToPath(new URL('../src/css/components/superactions.css', import.meta.url)),
        'utf8',
    );
    const declarations = css.replace(/\/\*[\s\S]*?\*\//g, '');
    assert.ok(!/grid-template-areas/.test(declarations), 'no grid-template-areas');
    assert.ok(!/grid-area:/.test(declarations), 'no per-tile grid-area');
    assert.ok(!/\.pos-/.test(declarations), 'no .pos-* placement rules');
    // The track counts come from the tiles, via custom properties the script
    // sets. A literal count here would be the same bug in a new spelling.
    assert.ok(/repeat\(var\(--ctl-cols/.test(declarations), 'columns come from --ctl-cols');
    assert.ok(/repeat\(var\(--ctl-rows/.test(declarations), 'rows come from --ctl-rows');
});
