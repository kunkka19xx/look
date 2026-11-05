import { copyToClipboard, clipboardPasteBlocker, pasteIntoFocusedApp, emojiSearch } from '../../ipc.js';
import { pushRecent, loadRecents, saveRecents } from '../../emoji-logic.js';

const LIMIT = 60;

let panel = null;
let input = null;
let feedback = null;
let list = null;
let recents = [];
let hits = [];
let selected = 0;
let token = 0;

export function init() {
    panel = document.getElementById('cmd-panel-emoji');
    input = document.getElementById('cmd-emoji-input');
    feedback = document.getElementById('cmd-emoji-feedback');
    list = document.getElementById('cmd-emoji-list');
    input.addEventListener('input', () => refresh());
    list.addEventListener('click', (e) => {
        const cell = e.target.closest('[data-index]');
        if (!cell) return;
        selected = Number(cell.dataset.index);
        render();
        activate(false);
    });
}

export function enter() {
    panel.hidden = false;
    input.value = '';
    recents = loadRecents(localStorage);
    refresh();
    requestAnimationFrame(() => input.focus());
}

export function exit() {
    panel.hidden = true;
}

async function refresh() {
    const query = input.value.trim();
    const mine = ++token;
    if (!query) {
        hits = recents;
        selected = 0;
        render();
        showFeedback(recents.length ? 'Recent' : 'Type to search emoji');
        return;
    }
    const found = await emojiSearch(query, LIMIT);
    if (mine !== token) return;
    hits = found;
    selected = 0;
    render();
    showFeedback(found.length ? `${found.length} found` : 'No emoji found');
}

function render() {
    list.replaceChildren(
        ...hits.map((hit, i) => {
            const cell = document.createElement('button');
            cell.type = 'button';
            cell.className = `cmd-emoji-cell${i === selected ? ' selected' : ''}`;
            cell.dataset.index = String(i);
            cell.title = hit.name;
            cell.textContent = hit.glyph;
            return cell;
        }),
    );
    const current = hits[selected];
    if (current) list.setAttribute('aria-label', current.name);
}

async function activate(insert) {
    const hit = hits[selected];
    if (!hit) return;
    await copyToClipboard(hit.glyph);
    recents = pushRecent(recents, { glyph: hit.glyph, name: hit.name });
    saveRecents(localStorage, recents);
    if (!insert) {
        showFeedback(`Copied ${hit.glyph}`);
        return;
    }
    const blocker = await clipboardPasteBlocker();
    if (blocker) {
        showFeedback(`Copied ${hit.glyph} instead: ${blocker}`, true);
        return;
    }
    await pasteIntoFocusedApp();
}

const STEPS = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 8, ArrowUp: -8 };

export function handleKey(e) {
    if (e.key === 'Enter') {
        e.preventDefault();
        activate(e.shiftKey);
        return true;
    }
    if (e.key in STEPS && hits.length) {
        e.preventDefault();
        selected = Math.min(hits.length - 1, Math.max(0, selected + STEPS[e.key]));
        render();
        return true;
    }
    return false;
}

export function showFeedback(text, isError = false) {
    feedback.textContent = text;
    feedback.className = `cmd-feedback ${isError ? 'cmd-feedback-error' : ''}`;
}
