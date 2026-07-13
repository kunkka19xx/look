// Quick Actions - the interactive part of the right panel (see
// docs/writing-controls.md). Descriptors for the selected result come from
// the shared core catalog; each action's live state/info and its execution go
// through the native adapter behind the qactions IPC commands. Mirrors macOS
// LauncherView+QuickActions: Ctrl+O flips the primary toggle, clicking the
// switch does the same, the outcome shows as a banner and state is re-read
// after apply. A stale-token guard drops late reads when the selection moves.

import { quickActions, quickActionState, quickActionApply, quickActionApplyItem } from '../ipc.js';
import * as banner from './banner.js';

// Banner durations (seconds), matching macOS Banner constants.
const BANNER_SUCCESS = 1.2;
const BANNER_ERROR = 1.6;
const BANNER_PERMISSION = 2.2;
// Matches the backend device-action timeout, so the "Connecting…" toast stays
// up for the whole wait and the outcome banner takes over when it lands.
const DEVICE_PENDING = 6;

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
// descriptor's info rows below it. Returns a handle used to feed async
// state/info updates into the DOM.
function buildAction(section, descriptor) {
    // Control row first, so the toggle sits directly under the header and the
    // status (with its per-device rows) reads beneath it.
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

    // Info fields (e.g. Status + connected devices) render below the control.
    // Each field owns a container the async status fills, so a value can render
    // as a single row (text) or a header plus one row per item (list).
    const infoFields = new Map();
    if (descriptor.info.length > 0) {
        const meta = document.createElement('div');
        meta.className = 'preview-meta';
        for (const field of descriptor.info) {
            const container = document.createElement('div');
            container.className = 'qaction-info-field';
            meta.appendChild(container);
            infoFields.set(field.value_key, { container, label: field.label });
        }
        section.appendChild(meta);
    }

    const handle = {
        descriptor,
        available: false,
        isOn: null,
        switchEl: null,
        controlWrap,
        infoFields,
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

    for (const [key, field] of handle.infoFields) {
        renderInfoField(handle, field, status.info[key]);
    }
}

// Fill one info field's container from its resolved value: a plain label/value
// row for text, or a labelled header plus one row per item for a list (e.g.
// each paired Bluetooth device), mirroring the folder listing. List items with
// an `id` are clickable and toggle that item (connect/disconnect a device).
function renderInfoField(handle, { container, label }, value) {
    container.innerHTML = '';
    if (value?.kind === 'list') {
        const connected = value.items.filter((it) => it.on === true).length;
        container.appendChild(
            infoRow(label, connected === 0 ? 'None connected' : `${connected} connected`),
        );
        const list = document.createElement('div');
        list.className = 'qaction-device-list';
        for (const item of value.items) {
            list.appendChild(deviceRow(handle, item));
        }
        container.appendChild(list);
        return;
    }
    const text = value?.kind === 'text' ? value.text : value?.reason || 'Unavailable';
    const row = infoRow(label, text);
    if (value?.kind !== 'text') {
        row.querySelector('.preview-info-value').classList.add('qaction-info-unavailable');
    }
    container.appendChild(row);
}

// One device row: a connection dot + name. When the item carries an `id` it is
// a clickable button that toggles that device's connection.
function deviceRow(handle, item) {
    const actionable = item.id != null;
    const row = document.createElement(actionable ? 'button' : 'div');
    row.className = 'qaction-device-row';
    row.classList.toggle('is-connected', item.on === true);
    if (actionable) {
        row.type = 'button';
        row.tabIndex = -1;
        row.addEventListener('click', () => runItem(handle, item));
    }
    row.appendChild(document.createElement('span')).className = 'qaction-device-dot';
    const nameEl = document.createElement('span');
    nameEl.className = 'qaction-device-name';
    nameEl.textContent = item.label;
    nameEl.title = item.label;
    row.appendChild(nameEl);
    return row;
}

// A label/value row matching the panel's other metadata rows.
function infoRow(label, value) {
    const row = document.createElement('div');
    row.className = 'preview-info-row';
    const labelEl = document.createElement('span');
    labelEl.className = 'preview-info-label';
    labelEl.textContent = label;
    row.appendChild(labelEl);
    const valueEl = document.createElement('span');
    valueEl.className = 'preview-info-value';
    valueEl.textContent = value;
    row.appendChild(valueEl);
    return row;
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

    await applyAndReconcile(handle, myToken, () =>
        quickActionApply(handle.descriptor.action_id, intent),
    );
}

// Toggle one list item (connect/disconnect a device), then reconcile as `run`.
async function runItem(handle, item) {
    if (inFlight) return;
    inFlight = true;
    const myToken = token;
    // Connecting can take a few seconds; show immediate feedback so the click
    // doesn't feel dead. The outcome banner replaces this when it lands.
    const connecting = item.on !== true;
    banner.show(
        `${connecting ? 'Connecting to' : 'Disconnecting'} ${item.label}…`,
        'info',
        DEVICE_PENDING,
    );
    await applyAndReconcile(handle, myToken, () =>
        quickActionApplyItem(handle.descriptor.action_id, item.id, 'toggle'),
    );
}

// Shared body for run/runItem: apply, show the outcome, and re-read state so the
// panel reflects reality. Everything is wrapped so a rejected IPC (backend
// error/panic, Tauri failure) still releases `inFlight` - otherwise the control
// wedges until the selection changes. Late responses (selection moved) drop on
// the token guard; `finally` only releases `inFlight` while we still own the
// current token (clear() already reset it for a newer run otherwise).
async function applyAndReconcile(handle, myToken, apply) {
    try {
        const outcome = await apply();
        if (token !== myToken) return;
        showOutcome(handle.descriptor, outcome);

        const keys = handle.descriptor.info.map((f) => f.value_key);
        const status = await quickActionState(handle.descriptor.action_id, keys);
        if (token !== myToken) return;
        applyStatus(handle, status);
    } catch (err) {
        console.error('quick action failed', err);
        if (token === myToken) showOutcome(handle.descriptor, null);
    } finally {
        if (token === myToken) inFlight = false;
    }
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
