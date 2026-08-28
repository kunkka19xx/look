import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The launchpad layout arrives from `invoke('launchpad_layout')` as the core
// serialised it, and this shell reads the keys RAW - `tile.action_id`, not
// `tile.actionId`. The macOS shell decodes the same JSON with
// `.convertFromSnakeCase` and so reads the camelCase spelling instead, which
// means the two shells never read the same string and a rename in the core
// breaks each of them separately and silently: a missing key here is
// `undefined`, which builds a tile with no id rather than throwing.
//
// The fixture is the one the Rust and Swift tests read. Regenerate all three
// together with:
//
//     UPDATE_FIXTURES=1 cargo test --manifest-path bridge/ffi/Cargo.toml

const FIXTURE = fileURLToPath(
    new URL('../../../bridge/ffi/tests/fixtures/launchpad_layout.json', import.meta.url),
);

const tiles = JSON.parse(readFileSync(FIXTURE, 'utf8'));

test('the core layout is not empty', () => {
    // `[]` is what a broken contract looks like from the outside, so this is
    // the assertion the rest depend on rather than a formality.
    assert.ok(Array.isArray(tiles), 'the layout is an array');
    assert.ok(tiles.length > 0, 'an empty layout is the failure mode, not a pass');
});

test('every tile carries the keys superactions.js dereferences', () => {
    for (const tile of tiles) {
        // buildTile/tileEl key off action_id for the DOM id, the icon lookup,
        // the mnemonic index and the click handler. Undefined here is a tile
        // that renders but answers to nothing.
        assert.equal(typeof tile.action_id, 'string', 'action_id');
        assert.ok(tile.action_id.length > 0, `${tile.action_id}: action_id is empty`);
        assert.equal(typeof tile.title, 'string', `${tile.action_id}: title`);
        assert.ok('mnemonic' in tile, `${tile.action_id}: mnemonic`);
        assert.ok('on_label' in tile, `${tile.action_id}: on_label`);
        assert.ok('off_label' in tile, `${tile.action_id}: off_label`);

        // Resolved placement. This shell does not read these YET - it still
        // places by CSS grid area (task #8 is the swap) - but the core sends
        // them now, and they are what the swap will consume. Pinning them here
        // means the geometry cannot rot in the gap between the two tasks.
        for (const key of ['col', 'row', 'col_span', 'row_span']) {
            assert.equal(typeof tile[key], 'number', `${tile.action_id}: ${key}`);
        }
        assert.ok(tile.col_span >= 1 && tile.row_span >= 1, `${tile.action_id} covers no cell`);
    }
});

test('the toggle tiles carry the captions their state line reads', () => {
    // `controls.set(..., { onLabel: tile.on_label ?? 'On' })`: the fallback
    // means a snake-case slip degrades to a generic On/Off rather than failing,
    // so nothing but a test notices.
    const theme = tiles.find((tile) => tile.action_id === 'theme');
    assert.ok(theme, 'the theme tile is in the layout');
    assert.equal(typeof theme.on_label, 'string');
    assert.equal(typeof theme.off_label, 'string');
});

test('every id in the layout has a grid area to be placed into', async () => {
    // AREA maps action_id -> the CSS grid-area in superactions.css. An id with
    // no entry keeps a plain tile and escapes its area, so it lands wherever
    // the grid auto-places it. This is the pairing that task #8 replaces with
    // resolved coordinates; until then it has to stay exhaustive.
    const source = readFileSync(
        fileURLToPath(new URL('../src/js/components/superactions.js', import.meta.url)),
        'utf8',
    );
    const block = source.match(/const AREA = \{([\s\S]*?)\};/);
    assert.ok(block, 'the AREA map is still in superactions.js');
    const mapped = new Set([...block[1].matchAll(/^\s*(\w+):/gm)].map((m) => m[1]));

    for (const tile of tiles) {
        assert.ok(mapped.has(tile.action_id), `${tile.action_id} has no grid area`);
    }
});
