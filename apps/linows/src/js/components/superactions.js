// Super actions - the empty-state "launchpad" control strip. When the query is
// empty on the home screen, `look` shows a compact bento of L/M/S tiles instead
// of a result list: the priority slot (todo / pomo / clock), quick toggles
// (Bluetooth, Wi-Fi, Theme, Keep Awake), info (Battery, Weather), one-shot
// system actions (Close All, Mic, Restart, Shut Down) and a Now Playing
// transport. Mirrors the macOS empty-state launchpad (issue #288).
//
// This module owns the UI and input routing. Live system state and the
// per-action execution adapters are wired later; every tile renders from a
// static snapshot, but each actionable tile already carries its mnemonic
// (Alt+<char>, the highlighted letter) and funnels both a click and the
// accelerator through activate() so the execution wiring lands in one place.
// Mirrors the macOS launchpad, where the same char fires on Cmd+<char>.

import {
    listChecks,
    clock,
    bluetooth,
    wifi,
    moon,
    battery,
    coffee,
    cloudSun,
    droplet,
    xCircle,
    mic,
    refreshCw,
    power,
    music,
    skipBack,
    skipForward,
    play,
} from '../icons.js';

let container = null;
let built = false;
let visible = false;

// Rebuilt on each render(): the actionable tiles keyed by id, and the map from
// an accelerator char (lowercased) to the id it fires. Both drive activate().
let tilesById = new Map();
let mnemonicIndex = new Map();

export function init(containerEl) {
    container = containerEl;
}

/**
 * Show or hide the control strip. Built lazily on first show so an app that
 * opens straight onto a query never pays for the DOM. Toggling the class on
 * the launcher window lets CSS trade the results row + hint bar for the strip.
 *
 * The reveal is animated (staggered tile fade-in); hiding is instant so typing
 * hands the space to the results list with no lag. A redundant show while
 * already visible is a no-op, so per-keystroke syncs don't replay the reveal.
 */
export function setVisible(show) {
    if (!container) return;
    if (show && !built) {
        render();
        built = true;
    }
    if (show === visible) return;
    visible = show;
    container.hidden = !show;
    document.documentElement.classList.toggle('controls-open', show);
    if (show) playEnter();
}

/**
 * Replay the entrance. Called when the window is re-summoned so the launchpad
 * animates in each time it appears, not only on first build.
 */
export function replayEnter() {
    if (visible) playEnter();
}

// Restart the staggered CSS animation: drop the class, force a reflow so the
// browser sees a clean start, then re-add it.
function playEnter() {
    container.classList.remove('is-entering');
    void container.offsetWidth;
    container.classList.add('is-entering');
}

export function isVisible() {
    return visible;
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
 * accelerator and a mouse click land here. The per-action execution adapters
 * are wired later (see the module header); for now activation pulses the tile
 * so the accelerator is visibly resolving to the right action.
 */
function activate(id) {
    const el = tilesById.get(id);
    if (!el) return false;
    flash(el);
    return true;
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

// Register an actionable tile: index it by id (and, when present, by its
// accelerator char) and make a click activate it, so mouse and keyboard share
// one path. The mnemonic char must be unique across tiles.
function bindAction(el, id, mnemonic) {
    tilesById.set(id, el);
    if (mnemonic) mnemonicIndex.set(mnemonic.toLowerCase(), id);
    el.addEventListener('click', () => activate(id));
}

// Wrap the first case-insensitive occurrence of the mnemonic char in a
// highlight span. Falls back to the plain label when the char is absent, so a
// tile never renders a broken accelerator. Labels are known static strings, so
// no HTML escaping is needed. Mirrors macOS mnemonicText.
function labelHTML(label, mnemonic) {
    if (!mnemonic) return label;
    const i = label.toLowerCase().indexOf(mnemonic.toLowerCase());
    if (i < 0) return label;
    return `${label.slice(0, i)}<span class="ctl-mnem">${label[i]}</span>${label.slice(i + 1)}`;
}

function render() {
    tilesById = new Map();
    mnemonicIndex = new Map();

    const grid = document.createElement('div');
    grid.className = 'control-strip-grid';

    // The mnemonic (final arg) is the highlighted, Alt-fired letter; it matches
    // the macOS launchpad chars where the action is the same. Chars are unique
    // (case-insensitively) so one press maps to one tile. Priority / Battery /
    // Weather / Now Playing carry none, as on macOS.
    grid.append(
        priorityTile(),
        toggleTile('bt', bluetooth, 'Bluetooth', 'On', true, 'B'),
        toggleTile('wifi', wifi, 'Wi-Fi', 'On', true, 'W'),
        powerInfoTile(),
        weatherTile(),
        toggleTile('theme', moon, 'Theme', 'Dark', true, 'T'),
        toggleTile('keep', coffee, 'Keep Awake', 'Off', false, 'K'),
        actionTile('scr', xCircle, 'Close All', 'danger', 'C'),
        actionTile('mic', mic, 'Mic', null, 'M'),
        actionTile('rst', refreshCw, 'Restart', 'danger', 'R'),
        actionTile('shut', power, 'Shut Down', 'danger', 'D'),
        nowPlayingTile(),
    );

    // Per-tile index drives the entrance stagger (CSS animation-delay).
    [...grid.children].forEach((el, i) => el.style.setProperty('--i', i));

    container.innerHTML = '';
    container.appendChild(grid);
}

// --- Tile builders ----------------------------------------------------------

// Base tile: a frosted card assigned to a named grid area. `variant` and
// `tone` add modifier classes (active toggle, danger action, ...).
function tile(area, variant, tone) {
    const el = document.createElement('button');
    el.type = 'button';
    el.tabIndex = -1;
    el.dataset.id = area;
    el.className = `ctl-tile ctl-tile--${variant} pos-${area}`;
    if (tone) el.classList.add(`is-${tone}`);
    return el;
}

function iconSpan(svg) {
    const el = document.createElement('span');
    el.className = 'ctl-icon';
    el.innerHTML = svg;
    return el;
}

// L slot (2x2): priority-driven, currently the Todo variant. Time + date, a
// `/todo` pill, the done/total tally and the next open task.
function priorityTile() {
    const el = tile('todo', 'priority');
    el.innerHTML = `
        <div class="ctl-priority-head">
            <span class="ctl-icon">${listChecks}</span>
            <div class="ctl-clock">
                <span class="ctl-clock-time">22:30</span>
                <span class="ctl-clock-date">Thu, Jul 23</span>
            </div>
            <span class="ctl-pill">/todo</span>
        </div>
        <div class="ctl-priority-body">
            <div class="ctl-priority-count"><b>0/3</b> done today</div>
            <div class="ctl-priority-next"><span class="ctl-dot"></span>Learning</div>
        </div>`;
    return el;
}

// M toggle (2x1): icon + label + on/off state. Active toggles carry the accent.
function toggleTile(area, svg, label, state, active, mnemonic) {
    const el = tile(area, 'toggle', active ? 'active' : null);
    el.appendChild(iconSpan(svg));
    const text = document.createElement('span');
    text.className = 'ctl-text';
    text.innerHTML = `<span class="ctl-label">${labelHTML(label, mnemonic)}</span><span class="ctl-state">${state}</span>`;
    el.appendChild(text);
    bindAction(el, area, mnemonic);
    return el;
}

// Adaptive info slot: Battery on a laptop, Uptime on a desktop that has no
// battery. The backend supplies `hasBattery` from its power-supply probe
// (Linux /sys/class/power_supply, Windows GetSystemPowerStatus); the static
// mock shows the battery case that matches the design reference.
function powerInfoTile(hasBattery = true) {
    return hasBattery
        ? infoTile('batt', battery, 'Battery', '100%')
        : infoTile('batt', clock, 'Uptime', '3d 4h');
}

// M info (2x1): small-caps label above a large value (Battery / Uptime).
function infoTile(area, svg, label, value) {
    const el = tile(area, 'info');
    el.appendChild(iconSpan(svg));
    const text = document.createElement('span');
    text.className = 'ctl-text';
    text.innerHTML = `<span class="ctl-caps">${label}</span><span class="ctl-value">${value}</span>`;
    el.appendChild(text);
    return el;
}

// L info (1x2): the weather stack - condition icon, temperature, condition and
// the high/low + humidity row.
function weatherTile() {
    const el = tile('weather', 'weather');
    el.innerHTML = `
        <span class="ctl-icon">${cloudSun}</span>
        <div class="ctl-weather-temp">27&deg;</div>
        <div class="ctl-caps">Partly Cloudy</div>
        <div class="ctl-weather-hl">H 36&deg; &nbsp; L 25&deg;</div>
        <div class="ctl-weather-hum">${droplet} 92%</div>`;
    return el;
}

// S action (1x1): centered icon + label, fires immediately (Restart/Shut Down
// carry the danger tone).
function actionTile(area, svg, label, tone, mnemonic) {
    const el = tile(area, 'action', tone);
    el.appendChild(iconSpan(svg));
    const name = document.createElement('span');
    name.className = 'ctl-label';
    name.innerHTML = labelHTML(label, mnemonic);
    el.appendChild(name);
    bindAction(el, area, mnemonic);
    return el;
}

// M media (span 3): track name + subtitle and the prev / play / next transport.
function nowPlayingTile() {
    const el = tile('play', 'media');
    el.innerHTML = `
        <span class="ctl-icon">${music}</span>
        <div class="ctl-text">
            <span class="ctl-label">Clouds - YTB</span>
            <span class="ctl-state">Look Dev</span>
        </div>
        <div class="ctl-transport">
            <span class="ctl-transport-btn">${skipBack}</span>
            <span class="ctl-transport-btn ctl-transport-play">${play}</span>
            <span class="ctl-transport-btn">${skipForward}</span>
        </div>`;
    return el;
}
