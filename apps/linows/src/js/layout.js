// Floating "inner-gap" layout. Port of the macOS LauncherView computed
// booleans (usesPanes / showsFloatingCards / hidesResultsForEmptyQuery /
// barFloatsFree / showsFloatingGrid). Every decision collapses into class
// toggles on the persistent .launcher-window; the tile DOM never rebuilds.
// WebKitGTK creates/destroys backdrop-filter surfaces on DOM churn, and the
// macOS equivalent froze typing when the gate depended on per-keystroke
// state - so the floating gate below reads coarse mode only (gap setting,
// command/settings/help), never query text or result counts.

import * as platform from './platform.js';

const GAP_MIN = 0;
const GAP_MAX = 24;

let innerGap = 0;
let queryEmpty = true;
let translateQuery = false;
let recentEmptyQuery = false;

// Modal screens that suspend the floating home layout entirely.
const modal = { command: false, settings: false, help: false };

let win = null;
let mainPane = null;

// Natural size of the configured background image + its ui_bg_layout mode,
// fed by settings.js. Needed to reproduce the classic backdrop's geometry
// (cover/center/stretch/tile, centered) when slicing the image per tile.
let bgImage = null;
let bgLayout = 'fill';

// Hint relocation targets (initHints). In the floating grid the hint text
// lives in the left card's footer and the copyright in the right card's;
// everywhere else both sit in the classic full-width bottom bar.
let hintBar = null;
let hintMessage = null;
let copyright = null;
let leftFooter = null;
let rightFooter = null;
let hintsInFooters = false;

export function init() {
    win = document.getElementById('app');
    mainPane = document.getElementById('search-content');

    // Tile origins feed the per-tile background-image slices: each tile paints
    // the panel-sized image shifted by its own offset, so adjacent tiles read
    // as one continuous picture cut apart by the transparent gaps. Observing
    // the row alongside the pane catches column show/hide without a resize.
    const ro = new ResizeObserver(() => updateTileOrigins());
    ro.observe(mainPane);
    ro.observe(document.getElementById('results-row'));
    for (const tile of mainPane.querySelectorAll('.pane-tile')) ro.observe(tile);
    apply();
}

export function initHints(refs) {
    ({ hintBar, hintMessage, copyright, leftFooter, rightFooter } = refs);
    apply();
}

export function setInnerGap(gap) {
    innerGap = Math.max(GAP_MIN, Math.min(GAP_MAX, Math.round(gap) || 0));
    document.documentElement.style.setProperty('--inner-gap', `${innerGap}px`);
    apply();
}

export function setModal(screen, on) {
    modal[screen] = !!on;
    apply();
}

export function setQuery({ empty, translate }) {
    queryEmpty = !!empty;
    translateQuery = !!translate;
    apply();
}

// `rc"` with nothing to show renders as one wide card, so the hint bar stays
// at the bottom instead of moving into card footers (macOS showsFloatingGrid
// excludes it). Safe to feed from live result counts: it only relocates two
// text nodes, never a blur surface.
export function setRecentEmpty(on) {
    recentEmptyQuery = !!on;
    apply();
}

export function setBackgroundImage(src) {
    bgImage = null;
    if (src) {
        const img = new Image();
        img.onload = () => {
            bgImage = { width: img.naturalWidth, height: img.naturalHeight };
            updateTileOrigins();
        };
        img.src = src;
    }
    updateTileOrigins();
}

export function setBackgroundLayout(mode) {
    bgLayout = mode || 'fill';
    updateTileOrigins();
}

// Rectangle the classic window backdrop draws the image into (the
// ui_bg_layout size mode, centered - mirrors .launcher-window::before with
// background-position: center), in window coordinates. Tiles slice this
// same rectangle so floating and classic render the image identically.
// Without a measured image, fall back to stretching across the window.
function drawnImageRect(winRect) {
    if (bgImage && bgLayout !== 'stretch') {
        let w = bgImage.width;
        let h = bgImage.height;
        if (bgLayout === 'fill') {
            const scale = Math.max(winRect.width / w, winRect.height / h);
            w *= scale;
            h *= scale;
        }
        return { x: (winRect.width - w) / 2, y: (winRect.height - h) / 2, w, h };
    }
    return { x: 0, y: 0, w: winRect.width, h: winRect.height };
}

// Re-evaluate the floating gate after environment state changes at runtime
// (the settings blur-fallback toggle flips data-disable-blur).
export function refresh() {
    apply();
}

function apply() {
    if (!win) return;
    // Degraded-rendering environments keep the classic framed panel: both the
    // gaps and the resting bar depend on real transparency. Still coarse -
    // platform info is static and the attribute only flips from settings.
    const supported = platform.floatingSupported();
    const onHome = !modal.command && !modal.settings && !modal.help;
    const floating = supported && innerGap > 0 && onHome; // showsFloatingCards
    const resting = supported && queryEmpty && onHome; // hidesResultsForEmptyQuery
    const barFree = floating || resting; // barFloatsFree
    const floatingGrid = floating && !translateQuery && !recentEmptyQuery;

    win.classList.toggle('floating', floating);
    win.classList.toggle('resting', resting);
    win.classList.toggle('bar-free', barFree);
    win.classList.toggle('floating-grid', floatingGrid);
    mainPane.classList.toggle('translating', translateQuery);
    placeHints(floatingGrid);
    if (floating) updateTileOrigins();
}

// The nodes MOVE between the bottom bar and the card footers (not clone),
// so there is a single source of hint text either way.
function placeHints(floatingGrid) {
    if (!hintBar || floatingGrid === hintsInFooters) return;
    hintsInFooters = floatingGrid;
    if (floatingGrid) {
        leftFooter.appendChild(hintMessage);
        rightFooter.appendChild(copyright);
    } else {
        hintBar.appendChild(hintMessage);
        hintBar.appendChild(copyright);
    }
}

function updateTileOrigins() {
    if (!win || !mainPane) return;
    const winRect = win.getBoundingClientRect();
    const img = drawnImageRect(winRect);
    win.style.setProperty('--bg-draw-w', `${img.w}px`);
    win.style.setProperty('--bg-draw-h', `${img.h}px`);
    // Queried fresh: the translate panel creates its tile lazily.
    for (const tile of mainPane.querySelectorAll('.pane-tile')) {
        const r = tile.getBoundingClientRect();
        tile.style.setProperty('--tile-x', `${r.left - winRect.left - img.x}px`);
        tile.style.setProperty('--tile-y', `${r.top - winRect.top - img.y}px`);
    }
}
