// Quick Actions - the interactive part of the right panel (see
// docs/writing-controls.md). Descriptors for the selected result come from
// the shared core catalog; each action's live state/info and its execution go
// through the native adapter behind the qactions IPC commands. Mirrors macOS
// LauncherView+QuickActions: Ctrl+O flips the primary toggle, clicking the
// switch does the same, the outcome shows as a banner and state is re-read
// after apply. A stale-token guard drops late reads when the selection moves.

import { quickActions, quickActionState, quickActionApply } from '../ipc.js';
import * as banner from './banner.js';

// Banner durations (seconds), matching macOS Banner constants.
const BANNER_SUCCESS = 1.2;
const BANNER_ERROR = 1.6;
const BANNER_PERMISSION = 2.2;

const VALUE_PLACEHOLDER = '…';
const TOGGLE_HINT = 'Ctrl+O';

let token = 0; // bumped on every render/clear; async work checks it
let primary = null; // handle of the first toggle action (drives Ctrl+O)
let inFlight = false; // debounce: one apply at a time

export function clear() {
    token += 1;
    primary = null;
    inFlight = false;
}

/**
 * Fetch the result's Quick Actions and append the section to `container`.
 * No-op for results the catalog declares nothing for (the common case).
 */
export async function render(container, result) {
    clear();
    const myToken = token;

    const descriptors = await quickActions(result.id, result.kind);
    if (token !== myToken || !descriptors?.length) return;

    const section = document.createElement('div');
    section.className = 'preview-qactions';

    for (const descriptor of descriptors) {
        const handle = buildAction(section, descriptor);
        if (descriptor.control === 'toggle' && !primary) primary = handle;
        loadStatus(handle, myToken);
    }

    container.appendChild(section);
}

/** Flip the selected result's primary toggle (Ctrl+O). */
export function togglePrimary() {
    if (primary?.available) run(primary, 'toggle');
}

// One action row: title, the control for its kind, key hint, plus the
// descriptor's info rows above it. Returns a handle used to feed async
// state/info updates into the DOM.
function buildAction(section, descriptor) {
    const infoValues = new Map();
    if (descriptor.info.length > 0) {
        const meta = document.createElement('div');
        meta.className = 'preview-meta';
        for (const field of descriptor.info) {
            const row = document.createElement('div');
            row.className = 'preview-info-row';
            const label = document.createElement('span');
            label.className = 'preview-info-label';
            label.textContent = field.label;
            row.appendChild(label);
            const value = document.createElement('span');
            value.className = 'preview-info-value';
            value.textContent = VALUE_PLACEHOLDER;
            row.appendChild(value);
            meta.appendChild(row);
            infoValues.set(field.value_key, value);
        }
        section.appendChild(meta);
    }

    const row = document.createElement('div');
    row.className = 'qaction-row';

    const title = document.createElement('span');
    title.className = 'qaction-title';
    title.textContent = descriptor.title;
    row.appendChild(title);

    const controlWrap = document.createElement('span');
    controlWrap.className = 'qaction-control';
    row.appendChild(controlWrap);
    section.appendChild(row);

    const handle = {
        descriptor,
        available: false,
        isOn: null,
        switchEl: null,
        controlWrap,
        infoValues,
    };

    if (descriptor.control === 'toggle') {
        const switchEl = document.createElement('button');
        switchEl.type = 'button';
        switchEl.className = 'qaction-toggle';
        switchEl.setAttribute('role', 'switch');
        switchEl.appendChild(document.createElement('span')).className = 'qaction-toggle-knob';
        switchEl.addEventListener('click', () => {
            if (handle.available) run(handle, 'toggle');
        });
        controlWrap.appendChild(switchEl);

        const hint = document.createElement('span');
        hint.className = 'qaction-hint';
        hint.textContent = TOGGLE_HINT;
        controlWrap.appendChild(hint);
        handle.switchEl = switchEl;
    } else {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'qaction-button';
        button.textContent = descriptor.title;
        button.addEventListener('click', () => {
            if (handle.available) run(handle, 'run');
        });
        controlWrap.appendChild(button);
    }

    return handle;
}

async function loadStatus(handle, myToken) {
    const keys = handle.descriptor.info.map((f) => f.value_key);
    const status = await quickActionState(handle.descriptor.action_id, keys);
    if (token !== myToken) return;
    applyStatus(handle, status);
}

function applyStatus(handle, status) {
    const { state } = status;
    if (state.state === 'unavailable') {
        handle.available = false;
        handle.controlWrap.innerHTML = '';
        const reason = document.createElement('span');
        reason.className = 'qaction-unavailable';
        reason.textContent = state.reason;
        handle.controlWrap.appendChild(reason);
    } else {
        handle.available = true;
        if (handle.switchEl && (state.state === 'on' || state.state === 'off')) {
            setSwitch(handle, state.state === 'on');
        }
    }

    for (const [key, el] of handle.infoValues) {
        const value = status.info[key];
        if (value?.kind === 'text') {
            el.textContent = value.text;
        } else {
            el.textContent = value?.reason || 'Unavailable';
            el.classList.add('qaction-info-unavailable');
        }
    }
}

function setSwitch(handle, on) {
    handle.isOn = on;
    handle.switchEl.setAttribute('aria-checked', String(on));
    handle.switchEl.classList.toggle('is-on', on);
}

// Run an action's intent (switch click, button click, or Ctrl+O), show the
// outcome, and re-read the state so the panel reflects what really happened.
async function run(handle, intent) {
    if (inFlight) return;
    inFlight = true;
    const myToken = token;

    // Flip a toggle immediately for instant feedback; the re-read below
    // confirms (and corrects it if the change did not take).
    if (intent === 'toggle' && handle.isOn != null) {
        setSwitch(handle, !handle.isOn);
    }

    const outcome = await quickActionApply(handle.descriptor.action_id, intent);
    if (token !== myToken) return;
    showOutcome(handle.descriptor, outcome);

    const keys = handle.descriptor.info.map((f) => f.value_key);
    const status = await quickActionState(handle.descriptor.action_id, keys);
    if (token !== myToken) return;
    applyStatus(handle, status);
    inFlight = false;
}

function showOutcome(descriptor, outcome) {
    switch (outcome?.outcome) {
        case 'ok':
            banner.show(outcome.banner || `${descriptor.title} done`, 'success', BANNER_SUCCESS);
            break;
        case 'needs_permission':
            banner.show(outcome.message, 'info', BANNER_PERMISSION);
            break;
        default:
            banner.show(outcome?.message || `${descriptor.title} failed`, 'error', BANNER_ERROR);
    }
}
