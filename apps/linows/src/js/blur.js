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

// Both backends take rectangles, so a rounded surface is its middle band plus
// one-pixel rows stepping around each corner. Rows sit inside the arc: blur
// past the curve would frost the transparent gap, while a sliver short of it
// hides under the anti-aliased edge.
//
// Logical pixels, the coordinate space Wayland's surface-local regions already
// speak; blur.rs scales them for X11, which has no per-window scale.
function rectsFor(el) {
    const { left, top, width, height } = layoutBox(el);
    if (width <= 0 || height <= 0) return [];
    const radius = parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0;
    const inset = Math.floor(Math.min(radius, width / 2, height / 2));
    if (inset <= 0) return [{ x: left, y: top, width, height }];

    const rects = [{ x: left, y: top + inset, width, height: height - inset * 2 }];
    // Row i spans [i, i + 1) from the edge; its outer side is the tighter one.
    const indentAt = (i) => Math.ceil(inset - Math.sqrt(inset * inset - (inset - i) ** 2));
    // Equal neighbours merge, so a large radius costs its steps, not its pixels.
    let start = 0;
    for (let i = 0; i < inset; i++) {
        const indent = indentAt(i);
        if (i + 1 < inset && indentAt(i + 1) === indent) continue;
        const rows = i + 1 - start;
        if (width > indent * 2) {
            const row = { x: left + indent, width: width - indent * 2, height: rows };
            rects.push({ ...row, y: top + start });
            rects.push({ ...row, y: top + height - start - rows });
        }
        start = i + 1;
    }
    return rects;
}

// Where the element settles, not where it is drawn this frame. The shell's
// arrival scales the panel and lifts the bar (motion.css), and a transform
// moves no layout, so a region measured mid-animation would never be corrected:
// ResizeObserver stays quiet and the frost is left stranded off the surface.
function layoutBox(el) {
    let left = 0;
    let top = 0;
    // Each offset is from the parent's padding edge, so its border comes along.
    for (let node = el; node; node = node.offsetParent) {
        left += node.offsetLeft + (node === el ? 0 : node.clientLeft);
        top += node.offsetTop + (node === el ? 0 : node.clientTop);
    }
    return { left, top, width: el.offsetWidth, height: el.offsetHeight };
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
        // Switched off, the empty region is what clears a standing request.
        const rects = platform.compositorBlurActive()
            ? surfaces().flatMap((el) => rectsFor(el))
            : [];
        const key = JSON.stringify(rects);
        if (key === lastKey) return;
        lastKey = key;
        setBlurRegion(rects);
    });
}
