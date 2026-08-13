// Compositor blur region. CSS cannot blur the desktop, so the real frost is a
// request to the compositor (platform/linux/blur.rs) and this only says which
// pixels. Inert where the request has no receiver: sync() returns on line one.
//
// The region tracks the painted surfaces, not the window: floating, the window
// is mostly holes, and blurring its rectangle would frost the gaps back into
// one slab.

import { setBlurRegion } from './ipc.js';
import * as platform from './platform.js';

let win = null;
let pending = false;
let lastKey = '';

export function init(rootEl) {
    win = rootEl;
}

// Both backends take rectangles, so a rounded surface is the cross of its two
// inset rects; the missed sliver hugs the outer curve.
//
// Logical pixels, the coordinate space Wayland's surface-local regions already
// speak; blur.rs scales them for X11, which has no per-window scale.
function rectsFor(el) {
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return [];
    const px = Math.round;
    const radius = parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0;
    const inset = Math.min(radius, r.width / 2, r.height / 2);
    if (inset <= 0) {
        return [{ x: px(r.left), y: px(r.top), width: px(r.width), height: px(r.height) }];
    }
    return [
        {
            x: px(r.left + inset),
            y: px(r.top),
            width: px(r.width - inset * 2),
            height: px(r.height),
        },
        {
            x: px(r.left),
            y: px(r.top + inset),
            width: px(r.width),
            height: px(r.height - inset * 2),
        },
    ];
}

// Classic is one surface; bar-free, every tile is its own.
function surfaces() {
    if (!win.classList.contains('bar-free')) return [win];
    return [...win.querySelectorAll('.pane-tile, .ctl-tile')].filter(
        (el) => el.offsetParent !== null,
    );
}

/** Recompute and send. Coalesced to one frame, and an unchanged region sends
 *  nothing, so callers can be as coarse or as frequent as they like. */
export function sync() {
    if (!win || !platform.compositorBlur() || pending) return;
    pending = true;
    requestAnimationFrame(() => {
        pending = false;
        const rects = surfaces().flatMap((el) => rectsFor(el));
        const key = JSON.stringify(rects);
        if (key === lastKey) return;
        lastKey = key;
        setBlurRegion(rects);
    });
}
