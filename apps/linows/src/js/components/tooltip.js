// Hover bubble for `[data-tip]` anchors, the web side of the macOS hoverBubble
// modifier. One element serves every anchor and it goes up on the first
// mouseover, no timer. Native `title=` is not usable here: GTK owns its delay,
// places it against the pointer, themes it itself, and in an undecorated
// WebKitGTK window it often never draws.

const GAP = 8; // anchor-to-bubble spacing, same as HoverBubbleModifier.gap
const MARGIN = 4; // closest the bubble gets to a window edge

let bubble = null;
let anchor = null;

const clamp = (v, lo, hi) => Math.min(Math.max(v, lo), hi);

function bubbleEl() {
    if (!bubble) {
        bubble = document.createElement('div');
        bubble.className = 'tip-bubble';
        document.body.appendChild(bubble);
    }
    return bubble;
}

// Transform rather than left/top so moving the bubble stays off the layout path.
function place(el, rect, trailing) {
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let x;
    let y;
    if (trailing) {
        x = rect.right + GAP;
        if (x + w > window.innerWidth - MARGIN) x = rect.left - GAP - w;
        y = rect.top + (rect.height - h) / 2;
    } else {
        x = rect.left + (rect.width - w) / 2;
        y = rect.top - h - GAP;
        if (y < MARGIN) y = rect.bottom + GAP;
    }
    x = clamp(x, MARGIN, window.innerWidth - w - MARGIN);
    y = clamp(y, MARGIN, window.innerHeight - h - MARGIN);
    el.style.transform = `translate3d(${Math.round(x)}px, ${Math.round(y)}px, 0)`;
}

// Shows the bubble for any `[data-tip]` under `container`, so anchors survive
// the innerHTML re-renders their panels do. `trailing` anchors it off the right
// edge, vertically centred, instead of above; `wide` picks the fixed-width
// wrapping bubble for sentence-length text.
export function attach(container, { trailing = false, wide = false } = {}) {
    container.addEventListener('mouseover', (e) => {
        const next = e.target.closest('[data-tip]');
        if (!next || next === anchor) return;
        anchor = next;
        const el = bubbleEl();
        el.textContent = next.dataset.tip;
        el.classList.toggle('tip-bubble-wide', wide);
        el.classList.add('tip-bubble-on');
        place(el, next.getBoundingClientRect(), trailing);
    });
    container.addEventListener('mouseout', (e) => {
        // Crossing into a child of the same anchor is not a leave.
        if (anchor && !anchor.contains(e.relatedTarget)) hide();
    });
}

// Panels call this when they close or swap pages: the pointer can be left
// resting on an anchor that is about to be hidden, and no mouseout follows.
export function hide() {
    if (!anchor) return;
    anchor = null;
    bubble.classList.remove('tip-bubble-on');
}
