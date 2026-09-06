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
// Every system tile is wired to a native adapter: the toggles (Bluetooth, Wi-Fi,
// Theme, Keep Awake), Battery, the Mic mute, and the one-shot buttons
// (Screensaver, Restart, Shut Down). The destructive buttons arm on first press
// and fire on the second, mirroring the macOS launchpad's inline confirm. The
// read-only Weather tile fills from the keyless IP-geo + Open-Meteo feed, and the
// Now Playing transport reads and drives the active MPRIS player; both refresh on
// summon (Now Playing also polls live while the strip is shown).

import {
    listChecks,
    timer,
    clock,
    bluetooth,
    wifi,
    moon,
    battery,
    batteryCharging,
    coffee,
    sun,
    cloudSun,
    cloud,
    cloudFog,
    cloudDrizzle,
    cloudRain,
    cloudSnow,
    cloudLightning,
    droplet,
    monitor,
    mic,
    micOff,
    refreshCw,
    power,
    music,
    skipBack,
    skipForward,
    play,
    pause,
} from '../icons.js';
import {
    launchpadLayout,
    launchpadTileValues,
    launchpadWarnings,
    refreshLaunchpadTiles,
    pressLaunchpadTile,
    quickActionState,
    quickActionApply,
    weatherCurrent,
    nowPlayingCurrent,
    nowPlayingCommand,
    lunarDate,
    todoList,
    systemUptime,
} from '../ipc.js';
import {
    snapshot as pomoSnapshot,
    formatTime as formatPomoTime,
    musicSnapshot,
    musicCommand,
} from '../screens/commands/pomo.js';
import * as platform from '../platform.js';
import * as banner from './banner.js';
import { gridPlacement, gridShape } from './launchpad-grid.js';

let container = null;
let built = false;
// What the user asked for, before the platform gets a say (see applyEnabled).
let configEnabled = true;
let visible = false;
// User setting (Settings -> Appearance -> Super Actions). When off the strip
// never shows and its accelerators never fire; setVisible collapses to hidden.
let enabled = true;

// The shared catalog layout: `{ tiles, columns, rows }`. Rendered from, never
// mutated. Fetched lazily and retried until it lands (see ensureLayout);
// layoutFetch is the in-flight request.
let layout = null;
let layoutFetch = null;
// True while a reveal is awaiting the layout, so a second caller (init vs a
// summon) doesn't run the reveal a second time and replay the animation.
let revealPending = false;

// Rebuilt on each render(): actionable tiles keyed by id, the accelerator-char
// -> id index, and the state-bearing controls (toggles + info) we re-read live.
let tilesById = new Map();
let mnemonicIndex = new Map();
let controls = new Map();

// The L-slot's persistent chrome refs (icon, header clock, pill, body) and its
// current sub-slot; plus the Todo tally and open-task rotation it reads.
let slotEls = null;
let todoStat = { done: 0, total: 0 };
let openTasks = [];
let taskCursor = 0;

// Today's lunar date ({ day, month, leap }) shown in the Clock slot header, from
// the shared look-lunar core crate, memoized by day key so the summon path only
// hits IPC when the date actually rolls over. Null until the first fetch.
let lunarToday = null;
let lunarKey = null;

// Weather and Now Playing tiles feed from external sources, not the qactions
// adapter registry, so they hold their own DOM refs and refresh tokens (a stale
// async read for a superseded summon / poll is dropped, independent of the
// control state token below).
let weatherEls = null;
let mediaEls = null;
let weatherToken = 0;
let customToken = 0;
let mediaToken = 0;
// 'internal' (pomo) or 'mpris': the source that last actually played. Breaks the
// tie when both are paused so the tile resumes whichever the user last used.
let mediaLastSource = null;

// Bumped on every refresh so a late async state read for a stale summon is
// dropped; guards against clobbering the optimistic value a press just set.
let stateToken = 0;
let applying = false;

// The armed destructive tile awaiting its confirming second press, or null.
let pendingConfirmId = null;
let confirmTimer = null;

// Clock re-render cadence, open-task rotation, and the live pomo countdown,
// matching macOS.
let clockTimer = null;
let taskTimer = null;
let pomoTimer = null;
let mediaTimer = null;
const CLOCK_TICK_MS = 20000;
const TASK_ROTATE_MS = 2600;
const POMO_TICK_MS = 1000;
// Now Playing changes out of band (track advances, user pauses elsewhere), so
// poll it live while the strip is shown. Weather is cached ~30 min, so it only
// refreshes on summon.
const MEDIA_TICK_MS = 2000;

// Backend WMO condition key -> tile glyph. clear/partly reuse the sun icons.
const WEATHER_ICON = {
    clear: sun,
    partly: cloudSun,
    cloudy: cloud,
    fog: cloudFog,
    drizzle: cloudDrizzle,
    rain: cloudRain,
    showers: cloudRain,
    snow: cloudSnow,
    thunder: cloudLightning,
};

// A forgotten prompt must not fire on a later stray press.
const CONFIRM_TIMEOUT_MS = 3000;

// Long enough to read a config error, which is longer than a toast.
const WARNING_SECONDS = 5;
const BATTERY_CHARGING_INFO_KEY = 'charging';
const BATTERY_CHARGING_INFO_TEXT = 'charging';
const CONTROL_INFO_KEYS = {
    battery: [BATTERY_CHARGING_INFO_KEY],
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
    // Prefetch the layout so the first show builds with no round trip; if the
    // strip was already asked to show before it arrived, build now.
    ensureLayout().then(() => {
        if (visible && !built) buildAndReveal();
    });

    // Pause the clock / rotation timers while the window is hidden; the reveal
    // and replayEnter restart them (see setVisible / replayEnter).
    document.addEventListener('visibilitychange', () => {
        if (document.hidden) stopTimers();
        else if (visible) startTimers();
    });
}

// Resolve the shared layout, fetching it on demand and retrying after a failure
// (the backend can be briefly unready at startup). Concurrent callers share one
// request. Resolves to whether the layout is now available.
function ensureLayout() {
    if (layout) return Promise.resolve(true);
    if (!layoutFetch) {
        layoutFetch = launchpadLayout()
            .then((resolved) => {
                layout = resolved;
                // Once per process, not per open: a broken drawing says so when
                // the launchpad first appears, without nagging on every summon.
                readWarnings().then(warningBanner);
            })
            .catch(() => {})
            .finally(() => {
                layoutFetch = null;
            });
    }
    return layoutFetch.then(() => !!layout);
}

/** Anything wrong with the drawing. Empty on the happy path and on failure. */
async function readWarnings() {
    try {
        return (await launchpadWarnings()) || [];
    } catch {
        return [];
    }
}

/**
 * Raise a drawing problem in the window, and say whether there was one - so a
 * caller with its own success banner knows to stay quiet.
 *
 * The count and the first message: a banner is not a log, and the rest are on
 * stderr.
 */
export function warningBanner(warnings) {
    const [first, ...rest] = warnings;
    if (!first) return false;
    banner.show(
        rest.length ? `${first} (+${rest.length} more)` : first,
        'warning',
        WARNING_SECONDS,
    );
    return true;
}

/**
 * Re-read the drawing, for the Ctrl+Shift+; config reload.
 *
 * The tiles are fetched once per process, which was right while the grid was a
 * compile-time constant. ~/.look/super-actions.toml decides it now, and arranging
 * tiles is an edit-and-look loop: without this an edit does nothing until the
 * app is restarted, which reads as the feature being broken.
 *
 * Returns the warnings rather than showing them, so the caller folds config and
 * launchpad problems into one banner.
 */
export async function reload() {
    if (!enabled) return [];
    let reloaded = null;
    try {
        reloaded = await launchpadLayout();
    } catch {
        return [];
    }
    // Most reloads are about something else and leave the drawing untouched.
    // Comparing first keeps those from re-reading every adapter for a grid that
    // did not move.
    if (reloaded.tiles.length && JSON.stringify(reloaded) !== JSON.stringify(layout)) {
        layout = reloaded;
        built = false;
        clearConfirm();
        if (visible) await buildAndReveal();
    }
    // Asked for even when nothing moved: that is exactly what a broken drawing
    // looks like, since it falls back to the default and the tiles never budge.
    return readWarnings();
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
    // A disabled strip is always hidden, so no summon can reveal it.
    if (!enabled) show = false;
    if (show === visible) return;
    visible = show;
    container.hidden = !show;
    document.documentElement.classList.toggle('controls-open', show);
    if (show) buildAndReveal();
    else {
        stopTimers();
        // Don't leave a Restart / Shut Down armed to fire on the next summon.
        clearConfirm();
    }
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
    if (!container || !visible) return;
    // Drop any in-flight entrance and pin the first-frame pose, then force a
    // synchronous reflow so the opacity-0 frame is painted before the window
    // actually hides (otherwise the compositor caches the fully-visible frame).
    container.classList.remove('is-entering');
    container.classList.add('is-armed');
    void container.offsetWidth;
}

export function isVisible() {
    return visible;
}

// Apply the Super Actions setting. Turning it off hides the strip immediately so
// its accelerators stop firing; turning it on lets the next syncControlStrip
// reveal it on the empty home screen.
export function setEnabled(on) {
    configEnabled = on;
    applyEnabled();
}

// Re-derive after something moved the floating gate at runtime - the blur
// fallback toggle is the one thing that does.
export function refreshAvailability() {
    applyEnabled();
}

// Same rule as the inner gap: the launchpad is the empty home screen's resting
// state, and a stack that cannot render that (platform.floatingSupported)
// shows the results list there instead. The config value is never touched, so
// the launchpad comes back by itself on a capable setup.
function applyEnabled() {
    const next = configEnabled && platform.floatingSupported();
    if (enabled === next) return;
    enabled = next;
    if (!next) setVisible(false);
}

export function isEnabled() {
    return enabled;
}

// Drop the built DOM so the next reveal rebuilds it. The stats block below the
// bento exists only where the panel stays opaque, and the blur toggle flips
// that (platform.floatingSupported) while the strip is already built.
export function invalidate() {
    built = false;
}

// Build (once) then reveal: read live state, start the timers, play the cascade.
// Fetches the layout first if needed, so a summon before/after a failed prefetch
// still builds instead of no-opping forever.
async function buildAndReveal() {
    if (!layout) {
        if (revealPending) return; // another caller is already awaiting the layout
        revealPending = true;
        const ready = await ensureLayout();
        revealPending = false;
        if (!ready) return; // retry on a later summon
    }
    if (!visible) return; // hidden again while the layout was in flight
    if (!built) {
        render(layout);
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
    pomoTimer = setInterval(tickPomo, POMO_TICK_MS);
    mediaTimer = setInterval(tickMedia, MEDIA_TICK_MS);
}

function stopTimers() {
    clearInterval(clockTimer);
    clearInterval(taskTimer);
    clearInterval(pomoTimer);
    clearInterval(mediaTimer);
    clockTimer = null;
    taskTimer = null;
    pomoTimer = null;
    mediaTimer = null;
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
 * accelerator and a mouse click land here. Routes by role: toggles flip, action
 * buttons fire (destructive ones via an inline confirm), the Mic action toggles
 * mute. An unwired tile just pulses. Mirrors the macOS launchpad's activate().
 */
function activate(id) {
    const el = tilesById.get(id);
    if (!el) return false;
    const ctl = controls.get(id);

    // A press on any tile other than the armed one clears a stale confirm, so
    // the danger prompt never lingers on an unrelated key.
    if (pendingConfirmId && pendingConfirmId !== id) clearConfirm();

    // Now Playing has no adapter control; its mnemonic toggles play/pause on the
    // active player, matching the macOS launchpad's Cmd+P.
    if (mediaEls && el === mediaEls.el) {
        flash(el);
        transport('playpause');
        return true;
    }

    // One gate, whatever the tile is: a first press on anything that asks arms
    // it, a second fires. Restart and Shut Down used to be named here by id.
    if (ctl?.confirm && pendingConfirmId !== id) {
        armConfirm(id, ctl);
        return true;
    }
    clearConfirm();

    // No adapter: the core holds the command and runs it by name.
    if (ctl?.role === 'custom') {
        if (!ctl.pressable) return true;
        flash(el);
        pressLaunchpadTile(id)
            .then((error) => {
                if (error) banner.show(error, 'error');
                else refreshCustomTiles((customToken += 1));
            })
            .catch(() => {});
        return true;
    }

    flash(el);
    if (!ctl?.wired) {
        // The adapter said why it cannot act (no screensaver service, no mic);
        // silence here reads as a dead key.
        if (ctl?.reason) banner.show(ctl.reason, 'info', 1.6);
        return true;
    }
    if (ctl.role === 'toggle') applyControl(id, ctl);
    else if (ctl.role === 'action') {
        if (ctl.toggleIntent) applyMic(id, ctl);
        else runAction(id, ctl);
    }
    return true;
}

// Arm a destructive tile: recolor it and swap the label to "Confirm?" until the
// second press or the auto-disarm timeout.
function armConfirm(id, ctl) {
    clearConfirm();
    pendingConfirmId = id;
    ctl.el.classList.add('is-confirming');
    ctl.labelEl.textContent = ctl.confirm || 'Confirm?';
    confirmTimer = setTimeout(clearConfirm, CONFIRM_TIMEOUT_MS);
}

// Drop any pending confirm and restore the tile's normal label.
function clearConfirm() {
    if (confirmTimer) {
        clearTimeout(confirmTimer);
        confirmTimer = null;
    }
    if (!pendingConfirmId) return;
    const ctl = controls.get(pendingConfirmId);
    if (ctl) {
        ctl.el.classList.remove('is-confirming');
        ctl.labelEl.innerHTML = labelHTML(ctl.title, ctl.mnemonic);
    }
    pendingConfirmId = null;
}

// Flip a wired toggle, show the outcome as a banner, and re-read the truth.
// Optimistic: flip immediately, reconcile after. One apply at a time so a double
// press can't race.
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

// Fire a one-shot action button (Screensaver, or a confirmed Restart / Shut
// Down) and surface its outcome. One at a time, like applyControl.
async function runAction(id, ctl) {
    if (applying) return;
    applying = true;
    try {
        showOutcome(ctl, await quickActionApply(id, 'run'));
    } catch (_) {
        showOutcome(ctl, null);
    } finally {
        applying = false;
    }
}

// Toggle the Mic action tile (an action-role tile with toggle semantics: it
// carries an off caption, so a press flips mute). Optimistic like applyControl.
async function applyMic(id, ctl) {
    if (applying) return;
    applying = true;
    // A muted tile presses to live and vice versa; resolve to an explicit target
    // so a stale panel still does what the user sees.
    const target = ctl.el.classList.contains('is-muted');
    setMicState(ctl, target);
    stateToken += 1;
    try {
        showOutcome(ctl, await quickActionApply(id, { set_on: target }));
    } catch (_) {
        showOutcome(ctl, null);
    } finally {
        applying = false;
        refreshControl(id, ctl, (stateToken += 1));
    }
}

// Reflect mic mute on its action tile: swap to the slashed-mic icon and tint it
// amber, matching the macOS launchpad (icon + colour, since the small action
// tile has no room for an On/Muted caption like the Bluetooth/Wi-Fi toggles).
function setMicState(ctl, live) {
    ctl.el.classList.toggle('is-muted', !live);
    if (ctl.iconEl) ctl.iconEl.innerHTML = live ? mic : micOff;
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

// Re-read everything the strip shows live: the L slot (pomo / todo / clock) and
// each wired control's state. Fire-and-forget; a stale-token guard drops late
// reads.
function refreshState() {
    renderSlot(); // reflect a running pomo / clock immediately
    refreshTodo(); // async; re-runs renderSlot once today's tasks land
    refreshLunar(); // async; fills the Clock slot header once it lands
    const myToken = (stateToken += 1);
    for (const [id, ctl] of controls) refreshControl(id, ctl, myToken);
    refreshWeather((weatherToken += 1));
    // Shares the summon token: a superseded open must not let a late
    // value land on tiles that have since been torn down.
    refreshCustomTiles((customToken += 1));
    refreshNowPlaying((mediaToken += 1));
}

// Fetch today's lunar date from core, but only when the day has rolled over
// since the last fetch (it changes only at midnight), then repaint whichever
// slot shows it: the prominent Clock-slot badge, or the dimmed header line the
// Todo / Pomo slots carry.
async function refreshLunar() {
    const key = todayKey();
    if (lunarKey !== key) {
        const now = new Date();
        try {
            lunarToday = await lunarDate(
                now.getFullYear(),
                now.getMonth() + 1,
                now.getDate(),
                -now.getTimezoneOffset() / 60,
            );
            lunarKey = key;
        } catch (_) {
            return;
        }
    }
    if (!slotEls) return;
    if (slotEls.slot === 'clock') writeLunar();
    else writeLunarLine();
}

async function refreshControl(id, ctl, myToken) {
    let status;
    try {
        status = await quickActionState(id, CONTROL_INFO_KEYS[ctl.actionId] || []);
    } catch (_) {
        return;
    }
    if (myToken !== stateToken) return;
    const s = status?.state;
    const wired = !!s && s.state !== 'unavailable';
    ctl.reason = wired ? null : s?.reason || null;
    if (ctl.role === 'toggle') {
        ctl.wired = wired;
        setToggleState(ctl, wired && s.state === 'on');
    } else if (ctl.role === 'info') {
        await refreshInfo(ctl, status, myToken);
    } else if (ctl.role === 'action') {
        ctl.wired = wired;
        // The Mic action tile has toggle semantics; reflect its live/muted state.
        if (ctl.toggleIntent) setMicState(ctl, wired && s.state === 'on');
    }
}

// The info tile shows Battery on a laptop; on a battery-less desktop the read is
// unavailable, so fall back to Uptime (relabelling the tile). Mirrors the older
// linows Battery/Uptime tile.
async function refreshInfo(ctl, status, myToken) {
    const s = status?.state;
    if (s?.state === 'value') {
        ctl.capsEl.textContent = ctl.title;
        ctl.valueEl.textContent = s.value;
        if (ctl.actionId === 'battery') {
            const charging =
                status?.info?.[BATTERY_CHARGING_INFO_KEY]?.kind === 'text' &&
                status.info[BATTERY_CHARGING_INFO_KEY].text === BATTERY_CHARGING_INFO_TEXT;
            ctl.iconEl.innerHTML = charging ? batteryCharging : battery;
        }
        return;
    }
    let uptime = null;
    try {
        uptime = await systemUptime();
    } catch (_) {}
    if (myToken !== stateToken) return;
    ctl.capsEl.textContent = uptime ? 'Uptime' : ctl.title;
    ctl.valueEl.textContent = uptime || '--';
    if (ctl.actionId === 'battery') {
        ctl.iconEl.innerHTML = battery;
    }
}

function setToggleState(ctl, on) {
    ctl.el.classList.toggle('is-active', on);
    ctl.stateEl.textContent = on ? ctl.onLabel : ctl.offLabel;
}

// --- L slot (pomo / todo / clock) -------------------------------------------
//
// The 2x2 slot is priority-driven, mirroring the macOS LaunchpadLSlotView:
// Pomo wins while a session is active, else Todo while tasks remain today, else
// a Clock fallback. Each sub-slot has its own icon, command pill and body; Todo
// and Pomo keep a compact clock in the header so the time stays visible, while
// the Clock slot owns the tile with a big time + date and puts today's lunar
// date in the header corner (where a second clock would just be redundant).

const SLOT_ICON = { pomo: timer, todo: listChecks, clock };

// Choose the winning sub-slot from live pomo + todo state.
function currentSlot() {
    if (pomoSnapshot()) return 'pomo';
    if (openTasks.length) return 'todo';
    return 'clock';
}

// Reflect the current sub-slot: swap chrome + body when it changes, then fill
// its live values. Safe to call before the slot is built.
function renderSlot() {
    if (!slotEls) return;
    const slot = currentSlot();
    if (slot !== slotEls.slot) switchSlot(slot);
    if (slot === 'pomo') fillPomo();
    else if (slot === 'todo') fillTodo();
    else fillClock();
}

// Swap icon and header content for the new sub-slot, rebuild its body, and
// crossfade. The header corner always shows something: the compact clock in the
// Todo / Pomo slots, the lunar date in the Clock slot (whose body owns the big
// clock). No command pill: the macOS L tile header is icon + clock only.
function switchSlot(slot) {
    slotEls.slot = slot;
    const clockSlot = slot === 'clock';
    slotEls.icon.innerHTML = SLOT_ICON[slot];
    slotEls.headClock.classList.toggle('is-lunar', clockSlot);
    slotEls.refs = SLOT_BODY[slot]();
    slotEls.body.classList.remove('is-slot-in');
    void slotEls.body.offsetWidth;
    slotEls.body.classList.add('is-slot-in');
}

// Live countdown + phase + progress for the running session.
function fillPomo() {
    const pomo = pomoSnapshot();
    if (!pomo) return renderSlot(); // session ended; re-pick the slot
    const r = slotEls.refs;
    r.time.textContent = formatPomoTime(pomo.secondsLeft);
    const phase = pomo.type === 'focus' ? 'Focus' : 'Break';
    r.sub.textContent = `${phase} - session ${pomo.index + 1}/${pomo.count}`;
    r.fill.style.width = `${Math.round(pomo.progress * 100)}%`;
    updateHeaderClock();
}

// Done/total today plus the rotating next open task.
function fillTodo() {
    const r = slotEls.refs;
    r.count.innerHTML = `<b>${todoStat.done}/${todoStat.total}</b> done today`;
    // Task names are user text: set as textContent, never HTML.
    r.next.textContent = openTasks.length ? openTasks[taskCursor % openTasks.length] : 'All clear';
    updateHeaderClock();
}

// Big time + date fallback in the body; the header corner carries the lunar
// date instead of a redundant second clock.
function fillClock() {
    writeClock(slotEls.refs.time, slotEls.refs.date);
    writeLunar();
}

// Paint today's lunar date into the Clock slot header (day/month, with the leap
// marker when the month repeats). Placeholder until the first fetch resolves.
function writeLunar() {
    if (!lunarToday) {
        slotEls.time.textContent = '--';
        slotEls.date.textContent = 'Lunar';
        return;
    }
    slotEls.time.textContent = `${lunarToday.day}/${lunarToday.month}`;
    slotEls.date.textContent = lunarToday.leap ? 'Lunar leap' : 'Lunar';
}

async function refreshTodo() {
    let tasks;
    try {
        tasks = await todoList();
    } catch (_) {
        return;
    }
    const today = todayKey();
    const mine = (tasks || []).filter((t) => t.due_date === today);
    todoStat = { done: mine.filter((t) => t.done).length, total: mine.length };
    openTasks = mine.filter((t) => !t.done).map((t) => t.name);
    taskCursor = 0;
    renderSlot();
}

// Rotate through the open tasks so a long day's list all gets a turn, matching
// the macOS launchpad. Only while the Todo slot shows, no-op with <= 1 task.
function rotateTask() {
    if (slotEls?.slot !== 'todo' || openTasks.length <= 1) return;
    taskCursor += 1;
    fillTodo();
}

// Advance the live pomo countdown each second while the Pomo slot shows.
function tickPomo() {
    if (slotEls?.slot === 'pomo') fillPomo();
}

// Poll the active MPRIS player so the Now Playing tile tracks changes made
// elsewhere (next track, paused in the browser) while the strip is shown.
function tickMedia() {
    if (mediaEls) refreshNowPlaying((mediaToken += 1));
}

// --- Weather (IP-geo + Open-Meteo feed) -------------------------------------

async function refreshWeather(myToken) {
    if (!weatherEls) return;
    let w;
    try {
        w = await weatherCurrent();
    } catch (_) {
        return;
    }
    // Superseded summon, tile torn down, or the feed had nothing: keep the last
    // reading rather than blanking it.
    if (myToken !== weatherToken || !weatherEls || !w) return;
    weatherEls.icon.innerHTML = WEATHER_ICON[w.symbol] || cloudSun;
    weatherEls.temp.textContent = w.temperature;
    weatherEls.caps.textContent = w.condition;
    // Feed values are backend-formatted numerics (e.g. "24°"), safe as HTML;
    // droplet is our own SVG. The nbsp keeps the H/L gap from collapsing.
    weatherEls.hl.innerHTML = `H ${w.high} &nbsp; L ${w.low}`;
    weatherEls.hum.innerHTML = `${droplet} ${w.rain_chance ?? '--%'}`;
}

// --- Now Playing (active MPRIS player) --------------------------------------

// The pomo background player is app-internal (rodio), so it never appears on
// MPRIS. Shape its snapshot like an MPRIS one for renderMedia.
function internalMedia(m) {
    return { title: m.track, artist: 'Pomodoro', is_playing: m.playing, internal: true };
}

async function refreshNowPlaying(myToken) {
    if (!mediaEls) return;
    const internal = musicSnapshot();
    // Pomo actively playing outranks any MPRIS player.
    if (internal?.playing) {
        mediaLastSource = 'internal';
        renderMedia(internalMedia(internal));
        return;
    }
    let np;
    try {
        np = await nowPlayingCurrent();
    } catch (_) {
        return;
    }
    if (myToken !== mediaToken || !mediaEls) return;
    // A real player that is actually playing beats a paused pomo track.
    if (np?.title && np.is_playing) {
        mediaLastSource = 'mpris';
        renderMedia(np);
        return;
    }
    // Nothing is playing: keep whichever source last played, so a paused pomo
    // does not hijack a just-paused MPRIS player (and vice versa). Fall back to
    // pomo when the MPRIS player is gone entirely.
    if (internal && (mediaLastSource !== 'mpris' || !np?.title)) {
        renderMedia(internalMedia(internal));
        return;
    }
    renderMedia(np);
}

// Reflect play/pause on the transport (the accent class + the button glyph).
function setPlaying(playing) {
    mediaEls.el.classList.toggle('is-playing', playing);
    mediaEls.playBtn.innerHTML = playing ? pause : play;
}

// Reflect a track (or its absence) and the play/pause state on the transport.
function renderMedia(np) {
    // Handle the transport drives, so it hits the shown player, not a re-pick.
    mediaEls.player = np?.player ?? null;
    mediaEls.internal = !!np?.internal;
    setPlaying(!!np?.is_playing);
    if (!np?.title) {
        mediaEls.label.textContent = 'Nothing playing';
        mediaEls.state.textContent = '';
        return;
    }
    // Track / artist are media-supplied text: set as textContent, never HTML.
    mediaEls.label.textContent = np.title;
    mediaEls.state.textContent = np.artist || np.app || '';
}

// Send a transport command to the active player. Play/Pause flips optimistically
// and defers to the poll; next/previous re-read to show the new track promptly.
async function transport(command) {
    // Internal pomo player: state flips synchronously, so re-read at once.
    if (mediaEls?.internal) {
        // Flip the glyph now so pausing feels instant; pausing then re-reads
        // MPRIS (an await), which would otherwise lag the glyph. The re-read
        // reconciles either way.
        if (command === 'playpause') {
            setPlaying(!mediaEls.el.classList.contains('is-playing'));
        }
        musicCommand(command);
        refreshNowPlaying((mediaToken += 1));
        return;
    }
    const player = mediaEls?.player ?? null;
    if (command === 'playpause' && mediaEls) {
        // Optimistic flip; roll back unless delivered. No re-read: MPRIS
        // PlaybackStatus lags the command and would flip back (flicker).
        const wasPlaying = mediaEls.el.classList.contains('is-playing');
        setPlaying(!wasPlaying);
        let delivered = false;
        try {
            delivered = await nowPlayingCommand(command, player);
        } catch (_) {}
        if (!delivered) setPlaying(wasPlaying);
        return;
    }
    // next / previous: re-read so the new track shows without waiting for the poll.
    try {
        await nowPlayingCommand(command, player);
    } catch (_) {}
    refreshNowPlaying((mediaToken += 1));
}

// Re-render whichever clock the current slot shows (header clock for Todo/Pomo,
// the body clock for the Clock slot).
function updateClock() {
    if (!slotEls) return;
    if (slotEls.slot === 'clock') writeClock(slotEls.refs.time, slotEls.refs.date);
    else updateHeaderClock();
}

function updateHeaderClock() {
    writeClock(slotEls.time, slotEls.date);
    writeLunarLine();
}

// The dimmed lunar line under the Todo / Pomo header clock, kept visible while
// those slots occupy the tile (matching the macOS header clock). The Clock slot
// shows lunar prominently via `is-lunar` instead, so this line is hidden there.
function writeLunarLine() {
    if (!slotEls?.lunarLine) return;
    slotEls.lunarLine.textContent = lunarToday
        ? `${lunarToday.day}/${lunarToday.month} ${lunarToday.leap ? 'Lunar leap' : 'Lunar'}`
        : '';
}

// Write the current local time + date into a time node and a date node.
function writeClock(timeEl, dateEl) {
    const now = new Date();
    timeEl.textContent = now.toLocaleTimeString([], {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
    });
    dateEl.textContent = now.toLocaleDateString([], {
        weekday: 'short',
        month: 'short',
        day: 'numeric',
    });
}

// Local yyyy-MM-dd, matching the todo store's date keys.
function todayKey() {
    const d = new Date();
    const p = (n) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

// --- Rendering --------------------------------------------------------------

function render(layout) {
    const { tiles } = layout;
    tilesById = new Map();
    mnemonicIndex = new Map();
    controls = new Map();
    slotEls = null;
    weatherEls = null;
    mediaEls = null;

    const grid = document.createElement('div');
    grid.className = 'control-strip-grid';

    // The grid is whatever the drawing in ~/.look/super-actions.toml reaches. The
    // CSS used to declare `grid-template-areas` and every tile's `grid-area`,
    // which meant the arrangement was written once in the core and again here,
    // and the two had to agree. The core resolves it now and this only draws.
    const shape = gridShape(tiles, layout);
    grid.style.setProperty('--ctl-cols', shape.columns);
    grid.style.setProperty('--ctl-rows', shape.rows);

    for (const tile of tiles) {
        const el = buildTile(tile);
        const at = gridPlacement(tile);
        el.style.gridColumn = at.column;
        el.style.gridRow = at.row;
        grid.appendChild(el);
    }

    // Per-tile index drives the entrance stagger (CSS animation-delay). The core
    // sends tiles in reading order, so this follows the screen rather than the
    // order names happen to appear in the drawing.
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
        case 'custom':
            return buildCustom(tile);
        default:
            // A role this build has never heard of. A bare tile keeps its cell
            // rather than escaping the grid, which is the same bargain the
            // shape fallback makes for a payload from an older core.
            return tileEl(tile.action_id, 'action');
    }
}

// Base tile: a frosted card tagged with the role variant (and a tone for
// danger/active modifiers). Placement is set by the caller from the tile's own
// coordinates, so nothing here needs to know which tile this is.
function tileEl(actionId, variant, tone) {
    const el = document.createElement('button');
    el.type = 'button';
    el.tabIndex = -1;
    el.dataset.id = actionId;
    el.className = `ctl-tile ctl-tile--${variant}`;
    if (tone) el.classList.add(`is-${tone}`);
    return el;
}

function iconSpan(svg) {
    const el = document.createElement('span');
    el.className = 'ctl-icon';
    el.innerHTML = svg || '';
    return el;
}

// Paint an icon into a span: a built-in glyph by name, or a user's own image,
// which the backend has already read off disk and inlined as a data URL. The
// file is drawn as a CSS mask rather than an <img> so it takes the tile's
// colour the way an inline glyph does, including the active and danger tints;
// the alternative arrives in its own palette and reads as pasted on. Nothing
// recognised leaves the span empty, which CSS then collapses.
function applyIcon(el, name, fallback = '') {
    if (!el) return;
    const glyph = ICON[name];
    const inlined = typeof name === 'string' && name.startsWith('data:');
    const src = !glyph && inlined ? name : null;
    el.innerHTML = glyph || (src ? '' : fallback);
    el.classList.toggle('ctl-icon--file', Boolean(src));
    if (src) el.style.setProperty('--ctl-icon-src', `url("${src}")`);
    else el.style.removeProperty('--ctl-icon-src');
}

// An icon span already carrying `name`, for the tiles built with one.
function iconSpanFor(name, fallback) {
    const el = iconSpan('');
    applyIcon(el, name, fallback);
    return el;
}

// Index a tile by id (and its accelerator char) so activate() / handleMnemonic()
// can find it. Registration is separate from click-wiring: the media tile is
// indexed (for Alt+P) but stays click-inert.
function indexTile(el, tile) {
    tilesById.set(tile.action_id, el);
    if (tile.mnemonic) mnemonicIndex.set(tile.mnemonic.toLowerCase(), tile.action_id);
}

// Index an actionable tile and make a click activate it, so mouse and keyboard
// share one path.
function bindActionable(el, tile) {
    indexTile(el, tile);
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

// L slot (2x2): the priority pomo / todo / clock tile. Builds the persistent
// chrome (icon badge, header clock, command pill) and an empty body; renderSlot
// fills and swaps the body for whichever sub-slot wins. No adapter; fed by
// renderSlot / refreshTodo / the timers.
function buildSlot(tile) {
    const el = tileEl(tile.action_id, 'priority');
    el.innerHTML = `
        <div class="ctl-priority-head">
            <span class="ctl-icon"></span>
            <div class="ctl-clock">
                <span class="ctl-clock-time">--:--</span>
                <span class="ctl-clock-date"></span>
                <span class="ctl-clock-lunar"></span>
            </div>
        </div>
        <div class="ctl-slot-body"></div>`;
    slotEls = {
        icon: el.querySelector('.ctl-icon'),
        headClock: el.querySelector('.ctl-clock'),
        time: el.querySelector('.ctl-clock-time'),
        date: el.querySelector('.ctl-clock-date'),
        lunarLine: el.querySelector('.ctl-clock-lunar'),
        body: el.querySelector('.ctl-slot-body'),
        slot: null,
        refs: null,
    };
    renderSlot();
    return el;
}

// Per-slot body builders. Each replaces the slot body and returns refs to the
// live nodes the fill functions write into. Keyed for switchSlot.
const SLOT_BODY = {
    pomo: buildPomoBody,
    todo: buildTodoBody,
    clock: buildClockBody,
};

function buildPomoBody() {
    slotEls.body.innerHTML = `
        <div class="ctl-slot-time">00:00</div>
        <div class="ctl-slot-sub"></div>
        <div class="ctl-slot-bar"><div class="ctl-slot-bar-fill"></div></div>`;
    return {
        time: slotEls.body.querySelector('.ctl-slot-time'),
        sub: slotEls.body.querySelector('.ctl-slot-sub'),
        fill: slotEls.body.querySelector('.ctl-slot-bar-fill'),
    };
}

function buildTodoBody() {
    slotEls.body.innerHTML = `
        <div class="ctl-priority-count"><b>0/0</b> done today</div>
        <div class="ctl-priority-next"><span class="ctl-dot"></span><span></span></div>`;
    return {
        count: slotEls.body.querySelector('.ctl-priority-count'),
        next: slotEls.body.querySelector('.ctl-priority-next span:last-child'),
    };
}

function buildClockBody() {
    slotEls.body.innerHTML = `
        <div class="ctl-slot-time">--:--</div>
        <div class="ctl-slot-date"></div>`;
    return {
        time: slotEls.body.querySelector('.ctl-slot-time'),
        date: slotEls.body.querySelector('.ctl-slot-date'),
    };
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
        actionId: tile.action_id,
        iconEl: el.querySelector('.ctl-icon'),
        capsEl: text.querySelector('.ctl-caps'),
        valueEl: text.querySelector('.ctl-value'),
    });
    return el;
}

// A tile the user declared in ~/.look/super-actions.toml. Same anatomy as the tiles
// beside it; the core runs the command and this only draws the result.
function buildCustom(tile) {
    // A tile that only acts is drawn like Mic and Screensaver: a glyph over a
    // name. A placeholder would be a permanent "--".
    if (!tile.has_value) {
        const el = tileEl(tile.action_id, 'action');
        el.appendChild(iconSpanFor(tile.icon, power));
        const label = document.createElement('span');
        label.className = 'ctl-label';
        label.innerHTML = labelHTML(tile.title, tile.mnemonic);
        el.appendChild(label);
        controls.set(tile.action_id, {
            role: 'custom',
            el,
            title: tile.title,
            actionId: tile.action_id,
            pressable: Boolean(tile.pressable),
            confirm: tile.confirm || null,
            mnemonic: tile.mnemonic || null,
            labelEl: label,
        });

        // Without this the tile has no tilesById entry, so activate() bails on
        // its first line and neither click nor mnemonic reaches it.
        bindActionable(el, tile);
        return el;
    }

    const el = tileEl(tile.action_id, 'custom');
    // One cell fits the headline alone.
    const roomy = tile.row_span > 1 || tile.col_span > 1;
    // Shows before the first reading lands; a reading's own icon replaces it.
    const icon = iconSpanFor(tile.icon);

    const text = document.createElement('span');
    text.className = 'ctl-text';
    text.innerHTML =
        `<span class="ctl-caps">${labelHTML(tile.title, tile.mnemonic)}</span>` +
        `<span class="ctl-value">--</span>` +
        (roomy
            ? '<span class="ctl-custom-caption"></span><span class="ctl-custom-lines"></span>'
            : '');

    if (roomy) {
        // Only the name shares the icon's row. The reading and its lines start
        // at the tile's edge rather than in a gutter the icon opened, which a
        // one-cell tile has no room to do and a tall one no reason to.
        const head = document.createElement('span');
        head.className = 'ctl-custom-head';
        head.appendChild(icon);
        head.appendChild(text.querySelector('.ctl-caps'));
        text.prepend(head);
        el.appendChild(text);
    } else {
        el.appendChild(icon);
        el.appendChild(text);
    }

    controls.set(tile.action_id, {
        role: 'custom',
        el,
        title: tile.title,
        actionId: tile.action_id,
        // A tile with no `press` is a readout.
        pressable: Boolean(tile.pressable),
        confirm: tile.confirm || null,
        // Decides whether the caption may replace the name.
        mnemonic: tile.mnemonic || null,
        // Kept so a reading with no icon of its own does not wipe it.
        icon: tile.icon || null,
        // Also the label an armed confirm writes into.
        labelEl: text.querySelector('.ctl-caps'),
        iconEl: el.querySelector('.ctl-icon'),
        valueEl: text.querySelector('.ctl-value'),
        captionEl: text.querySelector('.ctl-custom-caption'),
        linesEl: text.querySelector('.ctl-custom-lines'),
    });
    bindActionable(el, tile);
    return el;
}

// Two calls on purpose: the first reads a cache and returns at once, so the
// strip never waits on a command to paint; the second spawns.
async function refreshCustomTiles(myToken) {
    const custom = [...controls.values()].filter((c) => c.role === 'custom' && c.valueEl);
    if (custom.length === 0) return;

    const apply = (values) => {
        if (myToken !== customToken) return;
        for (const ctl of custom) {
            const v = values?.[ctl.actionId];
            // No entry: never run, or printed nothing - which hides the tile.
            ctl.el.hidden = !v;
            if (!v) continue;
            ctl.valueEl.textContent = v.value ?? '--';
            ctl.el.classList.toggle('is-active', (v.state || '').toLowerCase() === 'on');
            applyIcon(ctl.iconEl, v.icon || ctl.icon);
            // The command's caption wins over the tile's name, as Weather shows
            // the condition. Unless the tile has a key: that letter is in the name.
            if (!ctl.mnemonic && v.caption) {
                ctl.labelEl.textContent = v.caption;
            }
            if (ctl.captionEl) {
                ctl.captionEl.textContent = ctl.mnemonic ? v.caption || '' : '';
            }
            if (ctl.linesEl) {
                ctl.linesEl.innerHTML = '';
                for (const line of (v.lines || []).slice(0, 3)) {
                    const el = document.createElement('span');
                    el.textContent = line;
                    ctl.linesEl.appendChild(el);
                }
            }
        }
    };

    try {
        apply(await launchpadTileValues());
    } catch (_) {
        return; // a core too old to answer leaves the placeholders alone
    }

    try {
        const [refreshed, errors] = await refreshLaunchpadTiles();
        for (const message of errors || []) banner.show(message, 'error');
        // Only when something ran: most opens are inside every tile's window.
        if (refreshed > 0) apply(await launchpadTileValues());
    } catch (_) {
        // A failed refresh keeps whatever the tiles already showed.
    }
}

// L info (1x2): the weather stack. Filled from the external feed by
// refreshWeather; shows placeholder dashes until the first reading lands.
function buildWeather(tile) {
    const el = tileEl(tile.action_id, 'weather');
    el.innerHTML = `
        <span class="ctl-icon">${cloudSun}</span>
        <div class="ctl-weather-temp">--&deg;</div>
        <div class="ctl-caps">Weather</div>
        <div class="ctl-weather-hl">H --&deg; &nbsp; L --&deg;</div>
        <div class="ctl-weather-hum">${droplet} --%</div>`;
    weatherEls = {
        icon: el.querySelector('.ctl-icon'),
        temp: el.querySelector('.ctl-weather-temp'),
        caps: el.querySelector('.ctl-caps'),
        hl: el.querySelector('.ctl-weather-hl'),
        hum: el.querySelector('.ctl-weather-hum'),
    };
    return el;
}

// S action (1x1): centered icon + label. Fires its adapter on press: Screensaver
// one-shot, Restart / Shut Down via inline confirm, Mic as a mute toggle (it
// carries an off caption). Danger tone for the destructive ones.
function buildAction(tile) {
    const danger = Boolean(tile.confirm);
    const el = tileEl(tile.action_id, 'action', danger ? 'danger' : null);
    const icon = iconSpan(ICON[tile.action_id]);
    el.appendChild(icon);
    const name = document.createElement('span');
    name.className = 'ctl-label';
    name.innerHTML = labelHTML(tile.title, tile.mnemonic);
    el.appendChild(name);
    bindActionable(el, tile);
    controls.set(tile.action_id, {
        role: 'action',
        el,
        iconEl: icon,
        title: tile.title,
        mnemonic: tile.mnemonic,
        labelEl: name,
        // An off caption means the button flips state (Mic mute) rather than
        // firing once (Screensaver, Restart, Shut Down). Mirrors macOS.
        toggleIntent: tile.off_label != null,
        danger,
        // The question this tile asks before it fires, from the core.
        confirm: tile.confirm || null,
        wired: false,
    });
    return el;
}

// M media (span 3): track name + subtitle and the prev / play / next transport.
// Reads and drives the active MPRIS player; refreshNowPlaying fills it and the
// transport buttons send commands. The whole-tile has no action (the buttons do
// the work), so it is not registered as actionable.
function buildMedia(tile) {
    const el = tileEl(tile.action_id, 'media');
    el.innerHTML = `
        <span class="ctl-icon">${music}</span>
        <div class="ctl-text">
            <span class="ctl-label">Nothing playing</span>
            <span class="ctl-state"></span>
        </div>
        <div class="ctl-transport">
            <span class="ctl-transport-btn" data-cmd="previous">${skipBack}</span>
            <span class="ctl-transport-btn ctl-transport-play" data-cmd="playpause">${play}</span>
            <span class="ctl-transport-btn" data-cmd="next">${skipForward}</span>
        </div>`;
    mediaEls = {
        el,
        label: el.querySelector('.ctl-label'),
        state: el.querySelector('.ctl-state'),
        playBtn: el.querySelector('.ctl-transport-play'),
    };
    for (const btn of el.querySelectorAll('.ctl-transport-btn')) {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            transport(btn.dataset.cmd);
        });
    }
    // Index for the mnemonic (Alt+P toggles play/pause via activate()) without
    // wiring a whole-tile click: the transport buttons own the clicks.
    indexTile(el, tile);
    return el;
}
