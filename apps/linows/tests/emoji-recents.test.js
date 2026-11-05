import { test } from 'node:test';
import assert from 'node:assert/strict';
import { pushRecent, loadRecents, saveRecents, RECENTS_CAP } from '../src/js/emoji-logic.js';

const memory = (seed = {}) => {
    const data = { ...seed };
    return {
        getItem: (k) => (k in data ? data[k] : null),
        setItem: (k, v) => {
            data[k] = String(v);
        },
    };
};

const hit = (glyph) => ({ glyph, name: glyph });

test('recents list the newest first', () => {
    const out = pushRecent(pushRecent([], hit('A')), hit('B'));
    assert.deepEqual(
        out.map((e) => e.glyph),
        ['B', 'A'],
    );
});

test('reusing an emoji moves it to the front without duplicating it', () => {
    let list = [];
    for (const g of ['A', 'B', 'A']) list = pushRecent(list, hit(g));
    assert.deepEqual(
        list.map((e) => e.glyph),
        ['A', 'B'],
    );
});

test('recents are capped at 20 and drop the oldest', () => {
    let list = [];
    for (let i = 0; i < 21; i++) list = pushRecent(list, hit(`g${i}`));
    assert.equal(RECENTS_CAP, 20);
    assert.equal(list.length, 20);
    assert.equal(list[0].glyph, 'g20');
    assert.equal(list.at(-1).glyph, 'g1');
});

test('recents survive a restart through storage', () => {
    const storage = memory();
    saveRecents(storage, pushRecent([], hit('A')));
    assert.deepEqual(
        loadRecents(storage).map((e) => e.glyph),
        ['A'],
    );
});

test('corrupt or missing storage yields no recents', () => {
    assert.deepEqual(loadRecents(memory()), []);
    assert.deepEqual(loadRecents(memory({ 'look.emoji.recents': '{nope' })), []);
    assert.deepEqual(loadRecents(memory({ 'look.emoji.recents': '{"a":1}' })), []);
});

test('loaded recents drop malformed entries and respect the cap', () => {
    const many = Array.from({ length: 30 }, (_, i) => hit(`g${i}`));
    const raw = JSON.stringify([{ glyph: 1, name: 'n' }, { glyph: 'x' }, null, ...many]);
    const out = loadRecents(memory({ 'look.emoji.recents': raw }));
    assert.equal(out.length, 20);
    assert.ok(out.every((e) => typeof e.glyph === 'string' && e.glyph !== 'x'));
});
