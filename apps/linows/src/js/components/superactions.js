// Super actions - the empty-state "launchpad" control strip. When the query is
// empty on the home screen, `look` shows a compact bento of L/M/S tiles instead
// of a result list: the priority slot (todo / pomo / clock), quick toggles
// (Bluetooth, Wi-Fi, Theme, Keep Awake), info (Battery, Weather), one-shot
// system actions (Close All, Mic, Restart, Shut Down) and a Now Playing
// transport. Mirrors the macOS empty-state launchpad (issue #288).
//
// This module owns the UI only. Live system state and action execution are
// wired later; every tile currently renders from a static snapshot so the
// layout and styling can be reviewed on their own.

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

export function init(containerEl) {
    container = containerEl;
}

/**
 * Show or hide the control strip. Built lazily on first show so an app that
 * opens straight onto a query never pays for the DOM. Toggling the class on
 * the launcher window lets CSS trade the results row + hint bar for the strip.
 */
export function setVisible(show) {
    if (!container) return;
    if (show && !built) {
        render();
        built = true;
    }
    container.hidden = !show;
    document.documentElement.classList.toggle('controls-open', show);
}

export function isVisible() {
    return !!container && !container.hidden;
}

function render() {
    const grid = document.createElement('div');
    grid.className = 'control-strip-grid';

    grid.append(
        priorityTile(),
        toggleTile('bt', bluetooth, 'Bluetooth', 'On', true),
        toggleTile('wifi', wifi, 'Wi-Fi', 'On', true),
        infoTile('batt', battery, 'Battery', '100%'),
        weatherTile(),
        toggleTile('theme', moon, 'Theme', 'Dark', true),
        toggleTile('keep', coffee, 'Keep Awake', 'Off', false),
        actionTile('scr', xCircle, 'Close All', 'danger'),
        actionTile('mic', mic, 'Mic'),
        actionTile('rst', refreshCw, 'Restart', 'danger'),
        actionTile('shut', power, 'Shut Down', 'danger'),
        nowPlayingTile(),
    );

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
function toggleTile(area, svg, label, state, active) {
    const el = tile(area, 'toggle', active ? 'active' : null);
    el.appendChild(iconSpan(svg));
    const text = document.createElement('span');
    text.className = 'ctl-text';
    text.innerHTML = `<span class="ctl-label">${label}</span><span class="ctl-state">${state}</span>`;
    el.appendChild(text);
    return el;
}

// M info (2x1): small-caps label above a large value (Battery).
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
function actionTile(area, svg, label, tone) {
    const el = tile(area, 'action', tone);
    el.appendChild(iconSpan(svg));
    const name = document.createElement('span');
    name.className = 'ctl-label';
    name.textContent = label;
    el.appendChild(name);
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
