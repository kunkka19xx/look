import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isModifierOnly, specFor } from '../src/js/hotkeyspec.js';

function press(key, code, mods = {}) {
    return { key, code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods };
}

test('modifiers come first in a fixed order', () => {
    const e = press('K', 'KeyK', { shiftKey: true, ctrlKey: true, altKey: true, metaKey: true });
    assert.equal(specFor(e), 'ctrl+alt+shift+win+k');
});

test('a letter follows the printed layout, not the physical key', () => {
    // AZERTY: the key printed A sits where QWERTY has Q.
    assert.equal(specFor(press('a', 'KeyQ', { ctrlKey: true })), 'ctrl+a');
});

test('a non-Latin letter falls back to the physical key', () => {
    assert.equal(specFor(press('к', 'KeyR', { ctrlKey: true })), 'ctrl+r');
});

test('digits ignore Shift', () => {
    assert.equal(specFor(press('!', 'Digit1', { shiftKey: true, altKey: true })), 'alt+shift+1');
});

test('named and symbol keys use the lowercased code', () => {
    assert.equal(specFor(press(' ', 'Space', { altKey: true })), 'alt+space');
    assert.equal(specFor(press('`', 'Backquote', { ctrlKey: true })), 'ctrl+backquote');
    assert.equal(specFor(press('ArrowUp', 'ArrowUp', { ctrlKey: true })), 'ctrl+arrowup');
    assert.equal(specFor(press('F13', 'F13')), 'f13');
});

test('a lone modifier is not a shortcut yet', () => {
    assert.equal(isModifierOnly(press('Control', 'ControlLeft', { ctrlKey: true })), true);
    assert.equal(isModifierOnly(press('k', 'KeyK')), false);
});
