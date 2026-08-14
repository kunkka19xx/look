// Shell entrance motion. The window is only hidden and re-shown, never rebuilt,
// so the reveal replays on every summon instead of running once per process
// (macOS keys the same pass off appearanceRevealToken).
//
// Class toggles only; motion.css owns timing, curves and reduce-motion. The
// launchpad runs its own cascade (components/superactions.js) because it is
// built lazily.

const ARMED = 'is-armed';
const ENTERING = 'is-entering';

// A summon fires both window-shown and visibilitychange, a frame or two apart,
// and either can arrive first. The second call is the duplicate.
const REPLAY_GUARD_MS = 250;

let root = null;
let lastPlay = -Infinity;

export function init(rootEl) {
    root = rootEl;
}

// Restarting needs the classes dropped and a reflow forced.
export function playReveal() {
    if (!root) return;
    const now = performance.now();
    if (now - lastPlay < REPLAY_GUARD_MS) return;
    lastPlay = now;
    root.classList.remove(ARMED, ENTERING);
    void root.offsetWidth;
    root.classList.add(ENTERING);
}

// Hold the first frame while hidden: the stale buffer is presented a beat
// before window-shown runs, so a visible panel would flash then rewind.
export function armReveal() {
    if (!root) return;
    // A hide right after a reveal must still arm.
    lastPlay = -Infinity;
    root.classList.remove(ENTERING);
    root.classList.add(ARMED);
    void root.offsetWidth;
}
