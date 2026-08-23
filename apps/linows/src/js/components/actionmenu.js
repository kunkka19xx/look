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
import * as banner from './banner.js';

const EMPTY_BANNER = 'Nothing to do here';
// Said when the backend cannot answer what the row's verbs resolve to, so the
// chord does not read as a dead key.
const FAILED_BANNER = 'Could not read this row';
const BANNER_SECONDS = 1.2;
// Clear of the header, so the popup still reads as attached to the row above.
const HEADER_GAP = 8;
// Where to hang the menu when the preview has no header to hang it under.
const FALLBACK_TOP = 84;
// `aria-activedescendant` and `aria-controls` address their targets by id, so
// the list and its rows need stable ones.
const MENU_ID = 'action-menu';
const ROW_ID = (index) => `${MENU_ID}-row-${index}`;
// What a screen reader calls the list, which has no visible label of its own.
const MENU_LABEL = 'Actions';

// Ids of the two rows shown in place of the action list while a destructive
// target waits for an answer.
const CONFIRM_YES = 'srcconfirm:yes';
const CONFIRM_NO = 'srcconfirm:no';
const CONFIRM_CANCEL = 'Cancel';

let panel = null;
let input = null;
let menuEl = null;
let rows = [];
let focusedIndex = 0;
// Resolves the answer to the question the menu is asking right now, or null
// when it is not asking one. Escape and any close answer no, which is what
// makes Escape mean the safe thing.
let pendingConfirm = null;
// Bumped on every open and close, so a list still being resolved when the user
// changes their mind cannot open the menu behind them.
let token = 0;

/**
 * `inputEl` keeps DOM focus for as long as the menu is up, so typing goes on
 * filtering. It is what carries the menu's virtual focus for a screen reader:
 * `aria-controls` points at the list, `aria-activedescendant` at the row the
 * user is on.
 */
export function init(panelEl, inputEl) {
    panel = panelEl;
    input = inputEl;
}

function isOpen() {
    return menuEl != null;
}

/**
 * The vim half of the menu's bindings, shared with keyboard.js so the chord
 * that opens the menu and the chord that moves in it cannot drift apart.
 * Returns 'j', 'k', or null.
 */
export function vimKey(e) {
    if (!e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) return null;
    const key = e.key.toLowerCase();
    return key === 'j' || key === 'k' ? key : null;
}

export function close() {
    token += 1;
    // Closing IS the safe answer, whatever closed it.
    answer(false);
    if (!menuEl) return;
    input?.removeAttribute('aria-controls');
    input?.removeAttribute('aria-activedescendant');
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

    let descriptors;
    try {
        descriptors = await rowactions.descriptorsFor();
    } catch (err) {
        console.error('actionmenu: could not resolve actions', err);
        if (token === myToken) banner.show(FAILED_BANNER, 'error', BANNER_SECONDS);
        return;
    }
    if (token !== myToken) return;

    if (descriptors.length === 0) {
        banner.show(EMPTY_BANNER, 'info', BANNER_SECONDS);
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
    const vim = vimKey(e);
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

/**
 * Ask `question` in the menu rather than in a modal: the keys the user already
 * has (move, Enter, Escape) keep working and Escape means the safe thing.
 * Resolves true only if they pick the question itself.
 *
 * Exported because Enter on a row whose block declares `confirm` has to ask it
 * too, and asking it twice in two different shapes would be two things to
 * learn (specs/user-sources.md §2.5).
 */
export function askConfirm(question) {
    return new Promise((resolve) => {
        close();
        pendingConfirm = { resolve };
        mount([
            { id: CONFIRM_YES, title: question },
            { id: CONFIRM_NO, title: CONFIRM_CANCEL },
        ]);
        // Start on Cancel: a destructive action should cost one more press than
        // an accidental double-Enter.
        focusedIndex = 1;
        applyFocus();
    });
}

/** Settle an outstanding question, once. */
function answer(yes) {
    const pending = pendingConfirm;
    pendingConfirm = null;
    pending?.resolve(yes);
}

function mount(descriptors) {
    menuEl = document.createElement('div');
    menuEl.className = 'action-menu';
    menuEl.id = MENU_ID;
    // A list of choices rather than a menu bar: the row a user lands on is a
    // selection they then run, which is what `option` describes and what the
    // focused input is allowed to point at.
    menuEl.setAttribute('role', 'listbox');
    menuEl.setAttribute('aria-label', MENU_LABEL);
    menuEl.style.top = `${anchorTop()}px`;

    rows = descriptors.map((descriptor, index) => {
        const row = document.createElement('div');
        row.className = 'action-menu-row';
        row.id = ROW_ID(index);
        row.setAttribute('role', 'option');

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
        // The confirm question rides along: it is asked in the menu, so the
        // descriptor has to survive until the row is activated.
        return { id: descriptor.id, title: descriptor.title, confirm: descriptor.confirm, el: row };
    });

    // The menu hangs off the header, so a panel scrolled away from it would
    // open the menu out of sight.
    panel.scrollTop = 0;
    panel.appendChild(menuEl);
    input?.setAttribute('aria-controls', MENU_ID);
    // After the menu is in the document: applyFocus scrolls the focused row
    // into view, which a detached node cannot do.
    focusedIndex = 0;
    applyFocus();
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
        const focused = index === focusedIndex;
        el.classList.toggle('is-focused', focused);
        el.setAttribute('aria-selected', String(focused));
    });

    const focused = rows[focusedIndex]?.el;
    if (!focused) return;
    focused.scrollIntoView({ block: 'nearest' });
    // The row never takes DOM focus, so this is the only thing telling a screen
    // reader which one the keys are on.
    input?.setAttribute('aria-activedescendant', focused.id);
}

// The single entry point for running a row, by key or by click, so neither can
// take a shortcut the other does not: a click that bypassed this would skip the
// confirmation, so a `confirm` target would ask when reached with Enter and act
// silently when reached with the mouse.
function activate(index) {
    const row = rows[index];
    if (!row) return;

    // Answering a question. `close` settles it; picking the question itself is
    // the only yes.
    if (pendingConfirm) {
        const yes = row.id === CONFIRM_YES;
        answer(yes);
        close();
        return;
    }

    // Asking one. The menu swaps to the question, so the answer happens where
    // the user's eyes already are.
    if (row.confirm) {
        const id = row.id;
        askConfirm(row.confirm).then((yes) => {
            if (yes) rowactions.activate(id);
        });
        return;
    }

    close();
    rowactions.activate(row.id);
}
