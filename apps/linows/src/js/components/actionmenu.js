// Everything you can do to the selected row, in one popup (Ctrl+K).
//
// Mirrors macOS ActionMenuView: it floats under the preview's header rather
// than sitting in the layout, so a row with actions costs the preview no space
// until the user asks for them. The panel keeps its own controls (a Quick
// Action's toggle) and what they know; this lists the row's verbs.
//
// The list is built when the menu opens, not on every selection change: a
// row's verbs cost nothing until somebody looks at them.

import * as rowactions from './rowactions.js';
import * as results from './results.js';
import * as banner from './banner.js';

const EMPTY_BANNER = 'Nothing to do here';
const EMPTY_BANNER_SECONDS = 1.2;
// Clear of the header, so the popup still reads as attached to the row above.
const HEADER_GAP = 8;
// Where to hang the menu when the preview has no header to hang it under.
const FALLBACK_TOP = 84;

let panel = null;
let menuEl = null;
let rows = [];
let focusedIndex = 0;
// Bumped on every open and close, so a list still being resolved when the user
// changes their mind cannot open the menu behind them.
let token = 0;

export function init(panelEl) {
    panel = panelEl;
}

export function isOpen() {
    return menuEl != null;
}

export function close() {
    token += 1;
    if (!menuEl) return;
    menuEl.remove();
    menuEl = null;
    rows = [];
    focusedIndex = 0;
}

/**
 * Ctrl+K or Ctrl+J. A row with nothing to offer says so rather than showing an
 * empty box. Not a toggle: once the menu is up those two chords are its
 * movement keys, so Escape is what closes it.
 */
export async function open() {
    if (isOpen()) return;

    token += 1;
    const myToken = token;
    const descriptors = await descriptorsForSelection();
    if (token !== myToken) return;

    if (descriptors.length === 0) {
        banner.show(EMPTY_BANNER, 'info', EMPTY_BANNER_SECONDS);
        return;
    }
    mount(descriptors);
}

/**
 * The menu owns movement, Enter and Escape while it is up. Returns true when it
 * consumed the key, so the launcher's own bindings stay out of the way.
 */
export function handleKey(e) {
    if (!isOpen()) return false;

    // Ctrl+J / Ctrl+K move the way they do in every other list that takes vim
    // keys; the arrows do the same for everyone else.
    const vim = e.ctrlKey && !e.shiftKey && !e.altKey ? e.key.toLowerCase() : null;
    if (e.key === 'ArrowDown' || vim === 'j') {
        move(1);
    } else if (e.key === 'ArrowUp' || vim === 'k') {
        move(-1);
    } else if (e.key === 'Enter') {
        activate(focusedIndex);
    } else if (e.key === 'Escape') {
        close();
    } else {
        return false;
    }

    e.preventDefault();
    return true;
}

async function descriptorsForSelection() {
    const selected = results.getSelected();
    return selected ? rowactions.descriptorsFor(selected) : [];
}

function mount(descriptors) {
    menuEl = document.createElement('div');
    menuEl.className = 'action-menu';
    menuEl.style.top = `${anchorTop()}px`;

    rows = descriptors.map((descriptor, index) => {
        const row = document.createElement('div');
        row.className = 'action-menu-row';

        const title = document.createElement('span');
        title.className = 'action-menu-title';
        title.textContent = descriptor.title;
        row.appendChild(title);

        // The chord sits in its own column rather than trailing the label, so
        // the keys line up down the right edge and read as one list.
        const chord = document.createElement('span');
        chord.className = 'action-menu-key';
        chord.textContent = descriptor.chord || '';
        row.appendChild(chord);

        row.addEventListener('click', () => activate(index));
        menuEl.appendChild(row);
        return { descriptor, el: row };
    });

    focusedIndex = 0;
    applyFocus();
    // The menu hangs off the header, so a panel scrolled away from it would
    // open the menu out of sight.
    panel.scrollTop = 0;
    panel.appendChild(menuEl);
}

/** Flush under the preview header, whatever height that header came out. */
function anchorTop() {
    const header = panel.querySelector('.preview-header');
    return header ? header.offsetTop + header.offsetHeight + HEADER_GAP : FALLBACK_TOP;
}

// Wraps at both ends, so holding one direction cycles rather than dead-ends.
function move(offset) {
    const count = rows.length;
    if (count === 0) return;
    focusedIndex = (((focusedIndex + offset) % count) + count) % count;
    applyFocus();
}

function applyFocus() {
    rows.forEach(({ el }, index) => {
        el.classList.toggle('is-focused', index === focusedIndex);
    });
    rows[focusedIndex]?.el.scrollIntoView({ block: 'nearest' });
}

// The single entry point for running a row, by key or by click, so neither can
// take a shortcut the other does not.
function activate(index) {
    const row = rows[index];
    if (!row) return;

    close();
    rowactions.activate(row.descriptor.id);
}
