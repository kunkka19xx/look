import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The golden geometry of the empty-state launchpad, captured BEFORE placement
// moves into ~/.look/launchpad.toml.
//
// Both shells draw the same grid, but only this one states it machine-readably:
// superactions.css declares `grid-template-areas` and superactions.js maps each
// action_id onto an area. macOS composes the identical arrangement by hand out
// of nested stacks (EmptyStateLaunchpadView.swift:74-114), where it cannot be
// read back. So this file is the authority for what the layout IS, and the
// macOS renderer is checked against it once it too places by coordinate.
//
// What this protects: the resolver replaces the CSS areas with per-tile
// coordinates, and "the tile moved one column" is not something a passing build
// would otherwise tell anyone.

const root = new URL('../../../', import.meta.url);
const golden = JSON.parse(
    readFileSync(fileURLToPath(new URL('bridge/ffi/tests/fixtures/launchpad_geometry.json', root)), 'utf8'),
);
const css = readFileSync(
    fileURLToPath(new URL('../src/css/components/superactions.css', import.meta.url)),
    'utf8',
);
const js = readFileSync(
    fileURLToPath(new URL('../src/js/components/superactions.js', import.meta.url)),
    'utf8',
);

/** Every area name's bounding rectangle, read out of `grid-template-areas`. */
function placementFromCSS() {
    const block = css.match(/grid-template-areas:\s*((?:"[^"]*"\s*)+);/);
    assert.ok(block, 'superactions.css still declares grid-template-areas');
    const rows = [...block[1].matchAll(/"([^"]*)"/g)].map((m) => m[1].split(/\s+/).filter(Boolean));

    const areaOf = {};
    const map = js.match(/const AREA = \{([\s\S]*?)\};/);
    assert.ok(map, 'superactions.js still declares the AREA map');
    for (const [, id, area] of map[1].matchAll(/^\s*(\w+):\s*'(\w+)'/gm)) areaOf[area] = id;

    const box = {};
    rows.forEach((row, r) => {
        row.forEach((name, c) => {
            if (name === '.') return;
            const b = (box[name] ??= { c0: c, r0: r, c1: c, r1: r });
            b.c0 = Math.min(b.c0, c);
            b.r0 = Math.min(b.r0, r);
            b.c1 = Math.max(b.c1, c);
            b.r1 = Math.max(b.r1, r);
        });
    });

    const tiles = {};
    for (const [area, b] of Object.entries(box)) {
        assert.ok(areaOf[area], `grid area "${area}" is not mapped to an action_id`);
        tiles[areaOf[area]] = {
            col: b.c0,
            row: b.r0,
            col_span: b.c1 - b.c0 + 1,
            row_span: b.r1 - b.r0 + 1,
        };
    }
    return { columns: rows[0].length, rows: rows.length, tiles };
}

test('the rendered grid still matches the golden geometry', () => {
    assert.deepEqual(placementFromCSS(), golden);
});

test('the golden tiles the grid with no gap and no overlap', () => {
    // The property the current layout has and a user-declared one need not:
    // worth stating explicitly, because it is the thing that silently stops
    // being true once placement is editable.
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

test('the core places every tile exactly where the rendered grid does', () => {
    // The point of the whole exercise. The CSS above and the core below are two
    // independent statements of one layout, and until task #8 lands they are
    // both live: this shell still draws from the CSS while the core already
    // ships coordinates. If they ever disagree, the swap silently moves tiles.
    const layout = JSON.parse(
        readFileSync(fileURLToPath(new URL('bridge/ffi/tests/fixtures/launchpad_layout.json', root)), 'utf8'),
    );

    const fromCore = Object.fromEntries(
        layout.map((t) => [
            t.action_id,
            { col: t.col, row: t.row, col_span: t.col_span, row_span: t.row_span },
        ]),
    );

    assert.deepEqual(fromCore, golden.tiles);
});
