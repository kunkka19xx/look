// Super actions - the empty-state "launchpad" control strip. When the query is
// empty on the home screen, `look` shows a compact bento of L/M/S tiles instead
// of a result list: the priority slot (todo / pomo / clock), quick toggles
// (Bluetooth, Wi-Fi, Theme, Keep Awake), info (Battery), one-shot system actions
// (Screensaver, Mic, Restart, Shut Down) and a Now Playing transport.
//
// The tile set, order, sizes, roles, mnemonics and labels come from the shared
// `look-qactions` catalog (launchpad_layout) so every platform shell renders the
// same strip; only the live state reads differ. This module owns the DOM and
// input routing, and reads each control's state through the same qactions IPC
// the preview panel uses. Actionable tiles funnel a click and their Alt+<char>
// accelerator through activate(), mirroring the macOS launchpad's Cmd+<char>.
//
// Phase 1 wires the tiles with dependency-free backends: live Clock/date, Todo,
// Bluetooth, Theme, Battery. The remaining toggles/actions (Wi-Fi, Keep Awake,
// Screensaver, Mic, Restart, Shut Down) and the presentational Weather / Now
// Playing tiles render as placeholders until their adapters land, so an
// unwired tile just pulses on press instead of erroring.

import {
    listChecks,
    bluetooth,
    wifi,
    moon,
    battery,
    coffee,
    cloudSun,
    droplet,
    monitor,
    mic,
    refreshCw,
    power,
    music,
    skipBack,
    skipForward,
    play,
} from '../icons.js';
import {
    launchpadLayout,
    quickActionState,
    quickActionApply,
    todoList,
    systemUptime,
} from '../ipc.js';
import * as banner from './banner.js';

let container = null;
let built = false;
let visible = false;

// The shared catalog layout, fetched once. Rendered from, never mutated.
let layoutTiles = null;

// Rebuilt on each render(): actionable tiles keyed by id, the accelerator-char
// -> id index, and the state-bearing controls (toggles + info) we re-read live.
let tilesById = new Map();
let mnemonicIndex = new Map();
let controls = new Map();

// The L-slot's live sub-elements (clock + todo), and the open-task rotation.
let slotEls = null;
let openTasks = [];
let taskCursor = 0;

// Bumped on every refresh so a late async state read for a stale summon is
// dropped; guards against clobbering the optimistic value a press just set.
let stateToken = 0;
let applying = false;

// Clock re-render cadence and the open-task rotation, matching macOS.
let clockTimer = null;
let taskTimer = null;
const CLOCK_TICK_MS = 20000;
const TASK_ROTATE_MS = 2600;

// Destructive one-shot actions carry the danger tone.
const DANGER = new Set(['restart', 'shutdown']);

// action_id -> CSS grid-area suffix (pos-<area>) and glyph. The grid placement
// lives in superactions.css; this maps the shared ids onto it.
const AREA = {
    lslot: 'todo',
    bluetooth: 'bt',
    wifi: 'wifi',
    battery: 'batt',
    theme: 'theme',
    keepawake: 'keep',
    screensaver: 'scr',
    weather: 'weather',
    mic: 'mic',
    restart: 'rst',
    shutdown: 'shut',
    nowplaying: 'play',
};

const ICON = {
    bluetooth,
    wifi,
    theme: moon,
    keepawake: coffee,
    battery,
    screensaver: monitor,
    mic,
    restart: refreshCw,
    shutdown: power,
};

export function init(containerEl) {
    container = containerEl;
    // Prefetch the layout so the first show builds with no round trip. If the
    // strip was already asked to show before it arrived, build now.
    launchpadLayout()
        .then((tiles) => {
            layoutTiles = tiles;
            if (visible && !built) buildAndReveal();
        })
        .catch(() => {});

    // Pause the clock / rotation timers while the window is hidden; the reveal
    // and replayEnter restart them (see setVisible / replayEnter).
    document.addEventListener('visibilitychange', () => {
        if (document.hidden) stopTimers();
        else if (visible) startTimers();
    });
}

/**
 * Show or hide the control strip. Built lazily on first show from the shared
 * layout, so an app that opens straight onto a query never pays for the DOM.
 * Toggling the class on the launcher window lets CSS trade the results row +
 * hint bar for the strip.
 *
 * The reveal is animated (staggered tile fade-in); hiding is instant so typing
 * hands the space to the results list with no lag. A redundant show while
 * already visible is a no-op, so per-keystroke syncs don't replay the reveal.
 */
export function setVisible(show) {
    if (!container) return;
    if (show === visible) return;
    visible = show;
    container.hidden = !show;
    document.documentElement.classList.toggle('controls-open', show);
    if (show) buildAndReveal();
    else stopTimers();
}

/**
 * Replay the entrance. Called when the window is re-summoned so the launchpad
 * animates in each time it appears, re-reading live state (Bluetooth flipped
 * elsewhere, battery drained) so the strip is never stale.
 */
export function replayEnter() {
    if (!visible || !built) return;
    refreshState();
    startTimers();
    playEnter();
}

/**
 * Hold the strip at the cascade's first frame while the window is hidden.
 *
 * The window is shown (its stale buffer presented) a beat before the JS
 * window-shown handler runs replayEnter, so a strip left fully visible would
 * flash on screen and then visibly rewind to opacity 0. Arming it to the
 * entrance-start pose while hidden makes that stale frame match frame 0 of the
 * cascade, so the reveal reads as one continuous fade. No-op while visible.
 */
export function armEntrance() {
    if (container && visible) container.classList.add('is-armed');
}

export function isVisible() {
    return visible;
}

// Build (once) then reveal: read live state, start the timers, play the cascade.
function buildAndReveal() {
    if (!layoutTiles) return; // init() finishes the build when the layout lands
    if (!built) {
        render(layoutTiles);
        built = true;
    }
    refreshState();
    startTimers();
    playEnter();
}

// Restart the staggered CSS animation: drop the classes, force a reflow so the
// browser sees a clean start, then re-add is-entering. Clearing is-armed here
// hands the tiles straight from their held first-frame pose into the animation.
function playEnter() {
    container.classList.remove('is-armed', 'is-entering');
    void container.offsetWidth;
    container.classList.add('is-entering');
}

function startTimers() {
    stopTimers();
    clockTimer = setInterval(updateClock, CLOCK_TICK_MS);
    taskTimer = setInterval(rotateTask, TASK_ROTATE_MS);
}

function stopTimers() {
    clearInterval(clockTimer);
    clearInterval(taskTimer);
    clockTimer = null;
    taskTimer = null;
}

// --- Activation / mnemonics -------------------------------------------------

/**
 * Resolve an accelerator char to its tile and fire it. Returns false when no
 * tile owns the char so the caller can let the key fall through. Case
 * insensitive, mirroring the macOS Cmd+<char> launchpad (Alt+<char> here).
 */
export function handleMnemonic(char) {
    if (!char) return false;
    const id = mnemonicIndex.get(char.toLowerCase());
    return id ? activate(id) : false;
}

/**
 * Central dispatch for a super action, keyed by tile id: both the Alt+<char>
 * accelerator and a mouse click land here. Pulses the tile, then runs the
 * backing control when one is wired; an unwired tile just pulses (its adapter
 * lands in a later phase).
 */
function activate(id) {
    const el = tilesById.get(id);
    if (!el) return false;
    flash(el);
    const ctl = controls.get(id);
    if (ctl?.wired) applyControl(id, ctl);
    return true;
}

// Flip a wired toggle (only toggles are wired in this phase), show the outcome
// as a banner, and re-read the truth. Optimistic: flip immediately, reconcile
// after. One apply at a time so a double press can't race.
async function applyControl(id, ctl) {
    if (applying || ctl.role !== 'toggle') return;
    applying = true;

    // A press means "the opposite of what I'm looking at": resolve to an
    // explicit target so a stale panel (system changed while hidden) still does
    // what the user sees. Invalidate any in-flight reads so they can't clobber
    // this optimistic flip.
    const target = !ctl.el.classList.contains('is-active');
    setToggleState(ctl, target);
    stateToken += 1;

    try {
        const outcome = await quickActionApply(id, { set_on: target });
        showOutcome(ctl, outcome);
    } catch (_) {
        showOutcome(ctl, null);
    } finally {
        applying = false;
        refreshControl(id, ctl, (stateToken += 1));
    }
}

function showOutcome(ctl, outcome) {
    switch (outcome?.outcome) {
        case 'ok':
            banner.show(outcome.banner || `${ctl.title} done`, 'success', 1.2);
            break;
        case 'needs_permission':
            banner.show(outcome.message, 'info', 2.2);
            break;
        default:
            banner.show(outcome?.message || `${ctl.title} failed`, 'error', 1.6);
    }
}

// Keyboard activation has no :active, so pulse the tile to acknowledge the
// press. Restart the one-shot animation by clearing the class + forcing a
// reflow, then self-clear on animationend so a later press can replay it.
function flash(el) {
    el.classList.remove('is-pressing');
    void el.offsetWidth;
    el.classList.add('is-pressing');
    el.addEventListener('animationend', () => el.classList.remove('is-pressing'), {
        once: true,
    });
}

// --- Live state -------------------------------------------------------------

// Re-read everything the strip shows live: clock, today's todo, and each wired
// control's state. Fire-and-forget; a stale-token guard drops late reads.
function refreshState() {
    updateClock();
    refreshTodo();
    const myToken = (stateToken += 1);
    for (const [id, ctl] of controls) refreshControl(id, ctl, myToken);
}

async function refreshControl(id, ctl, myToken) {
    let status;
    try {
        status = await quickActionState(id, []);
    } catch (_) {
        return;
    }
    if (myToken !== stateToken) return;
    const s = status?.state;
    if (ctl.role === 'toggle') {
        ctl.wired = !!s && s.state !== 'unavailable';
        setToggleState(ctl, ctl.wired && s.state === 'on');
    } else if (ctl.role === 'info') {
        await refreshInfo(ctl, s, myToken);
    }
}

// The info tile shows Battery on a laptop; on a battery-less desktop the read is
// unavailable, so fall back to Uptime (relabelling the tile). Mirrors the older
// linows Battery/Uptime tile.
async function refreshInfo(ctl, s, myToken) {
    if (s?.state === 'value') {
        ctl.capsEl.textContent = ctl.title;
        ctl.valueEl.textContent = s.value;
        return;
    }
    let uptime = null;
    try {
        uptime = await systemUptime();
    } catch (_) {}
    if (myToken !== stateToken) return;
    ctl.capsEl.textContent = uptime ? 'Uptime' : ctl.title;
    ctl.valueEl.textContent = uptime || '--';
}

function setToggleState(ctl, on) {
    ctl.el.classList.toggle('is-active', on);
    ctl.stateEl.textContent = on ? ctl.onLabel : ctl.offLabel;
}

// --- L slot (clock + todo) --------------------------------------------------

function updateClock() {
    if (!slotEls) return;
    const now = new Date();
    slotEls.time.textContent = now.toLocaleTimeString([], {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
    });
    slotEls.date.textContent = now.toLocaleDateString([], {
        weekday: 'short',
        month: 'short',
        day: 'numeric',
    });
}

async function refreshTodo() {
    if (!slotEls) return;
    let tasks;
    try {
        tasks = await todoList();
    } catch (_) {
        return;
    }
    const today = todayKey();
    const mine = (tasks || []).filter((t) => t.due_date === today);
    const done = mine.filter((t) => t.done).length;
    slotEls.count.innerHTML = `<b>${done}/${mine.length}</b> done today`;
    openTasks = mine.filter((t) => !t.done).map((t) => t.name);
    taskCursor = 0;
    renderTask();
}

// Rotate through the open tasks so a long day's list all gets a turn, matching
// the macOS launchpad. No-op with 0 or 1 open task.
function rotateTask() {
    if (openTasks.length > 1) {
        taskCursor += 1;
        renderTask();
    }
}

function renderTask() {
    if (!slotEls) return;
    slotEls.next.textContent = '';
    slotEls.next.appendChild(document.createElement('span')).className = 'ctl-dot';
    const name = document.createElement('span');
    // Task names are user text: set as textContent, never HTML.
    name.textContent = openTasks.length ? openTasks[taskCursor % openTasks.length] : 'All clear';
    slotEls.next.appendChild(name);
}

// Local yyyy-MM-dd, matching the todo store's date keys.
function todayKey() {
    const d = new Date();
    const p = (n) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

// --- Rendering --------------------------------------------------------------

function render(tiles) {
    tilesById = new Map();
    mnemonicIndex = new Map();
    controls = new Map();
    slotEls = null;

    const grid = document.createElement('div');
    grid.className = 'control-strip-grid';
    for (const tile of tiles) grid.appendChild(buildTile(tile));

    // Per-tile index drives the entrance stagger (CSS animation-delay).
    [...grid.children].forEach((el, i) => el.style.setProperty('--i', i));

    container.innerHTML = '';
    container.appendChild(grid);
}

function buildTile(tile) {
    switch (tile.role) {
        case 'slot':
            return buildSlot(tile);
        case 'toggle':
            return buildToggle(tile);
        case 'info':
            return buildInfo(tile);
        case 'weather':
            return buildWeather(tile);
        case 'action':
            return buildAction(tile);
        case 'media':
            return buildMedia(tile);
        default:
            return tileEl(tile.action_id, 'action');
    }
}

// Base tile: a frosted card placed into its named grid area, tagged with the
// role variant (and a tone for danger/active modifiers).
function tileEl(actionId, variant, tone) {
    const el = document.createElement('button');
    el.type = 'button';
    el.tabIndex = -1;
    el.dataset.id = actionId;
    el.className = `ctl-tile ctl-tile--${variant} pos-${AREA[actionId]}`;
    if (tone) el.classList.add(`is-${tone}`);
    return el;
}

function iconSpan(svg) {
    const el = document.createElement('span');
    el.className = 'ctl-icon';
    el.innerHTML = svg;
    return el;
}

// Index an actionable tile by id (and its accelerator char) and make a click
// activate it, so mouse and keyboard share one path.
function bindActionable(el, tile) {
    tilesById.set(tile.action_id, el);
    if (tile.mnemonic) mnemonicIndex.set(tile.mnemonic.toLowerCase(), tile.action_id);
    el.addEventListener('click', () => activate(tile.action_id));
}

// Wrap the first case-insensitive occurrence of the mnemonic char in a
// highlight span. Falls back to the plain label when the char is absent. Labels
// are shared catalog strings, so no HTML escaping is needed. Mirrors macOS.
function labelHTML(label, mnemonic) {
    if (!mnemonic) return label;
    const i = label.toLowerCase().indexOf(mnemonic.toLowerCase());
    if (i < 0) return label;
    return `${label.slice(0, i)}<span class="ctl-mnem">${label[i]}</span>${label.slice(i + 1)}`;
}

// L slot (2x2): clock + date, a /todo pill, today's done/total tally and the
// rotating next open task. Purely presentational (no adapter); fed by
// updateClock / refreshTodo.
function buildSlot(tile) {
    const el = tileEl(tile.action_id, 'priority');
    el.innerHTML = `
        <div class="ctl-priority-head">
            <span class="ctl-icon">${listChecks}</span>
            <div class="ctl-clock">
                <span class="ctl-clock-time">--:--</span>
                <span class="ctl-clock-date"></span>
            </div>
            <span class="ctl-pill">/todo</span>
        </div>
        <div class="ctl-priority-body">
            <div class="ctl-priority-count"><b>0/0</b> done today</div>
            <div class="ctl-priority-next"><span class="ctl-dot"></span>All clear</div>
        </div>`;
    slotEls = {
        time: el.querySelector('.ctl-clock-time'),
        date: el.querySelector('.ctl-clock-date'),
        count: el.querySelector('.ctl-priority-count'),
        next: el.querySelector('.ctl-priority-next'),
    };
    return el;
}

// M toggle (1 col): icon + label + on/off state. Active toggles carry the
// accent. State is filled by refreshControl.
function buildToggle(tile) {
    const el = tileEl(tile.action_id, 'toggle');
    el.appendChild(iconSpan(ICON[tile.action_id]));
    const text = document.createElement('span');
    text.className = 'ctl-text';
    const offLabel = tile.off_label ?? 'Off';
    text.innerHTML = `<span class="ctl-label">${labelHTML(tile.title, tile.mnemonic)}</span><span class="ctl-state">${offLabel}</span>`;
    el.appendChild(text);
    bindActionable(el, tile);
    controls.set(tile.action_id, {
        role: 'toggle',
        el,
        title: tile.title,
        onLabel: tile.on_label ?? 'On',
        offLabel,
        stateEl: text.querySelector('.ctl-state'),
        wired: false,
    });
    return el;
}

// M info (1 col): small-caps label above a large value (Battery). Read-only;
// value filled by refreshControl.
function buildInfo(tile) {
    const el = tileEl(tile.action_id, 'info');
    el.appendChild(iconSpan(ICON[tile.action_id]));
    const text = document.createElement('span');
    text.className = 'ctl-text';
    text.innerHTML = `<span class="ctl-caps">${tile.title}</span><span class="ctl-value">--</span>`;
    el.appendChild(text);
    controls.set(tile.action_id, {
        role: 'info',
        el,
        title: tile.title,
        capsEl: text.querySelector('.ctl-caps'),
        valueEl: text.querySelector('.ctl-value'),
    });
    return el;
}

// L info (1x2): the weather stack. Placeholder until the weather feed lands.
function buildWeather(tile) {
    const el = tileEl(tile.action_id, 'weather');
    el.innerHTML = `
        <span class="ctl-icon">${cloudSun}</span>
        <div class="ctl-weather-temp">--&deg;</div>
        <div class="ctl-caps">Weather</div>
        <div class="ctl-weather-hl">H --&deg; &nbsp; L --&deg;</div>
        <div class="ctl-weather-hum">${droplet} --%</div>`;
    return el;
}

// S action (1x1): centered icon + label, fires immediately. Danger tone for the
// destructive ones. Unwired until its adapter lands, so a press just pulses.
function buildAction(tile) {
    const el = tileEl(tile.action_id, 'action', DANGER.has(tile.action_id) ? 'danger' : null);
    el.appendChild(iconSpan(ICON[tile.action_id]));
    const name = document.createElement('span');
    name.className = 'ctl-label';
    name.innerHTML = labelHTML(tile.title, tile.mnemonic);
    el.appendChild(name);
    bindActionable(el, tile);
    return el;
}

// M media (span 3): track name + subtitle and the prev / play / next transport.
// Placeholder until the Now Playing feed lands.
function buildMedia(tile) {
    const el = tileEl(tile.action_id, 'media');
    el.innerHTML = `
        <span class="ctl-icon">${music}</span>
        <div class="ctl-text">
            <span class="ctl-label">Nothing playing</span>
            <span class="ctl-state"></span>
        </div>
        <div class="ctl-transport">
            <span class="ctl-transport-btn">${skipBack}</span>
            <span class="ctl-transport-btn ctl-transport-play">${play}</span>
            <span class="ctl-transport-btn">${skipForward}</span>
        </div>`;
    bindActionable(el, tile);
    return el;
}
