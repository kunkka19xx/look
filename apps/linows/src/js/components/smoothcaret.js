// Smooth caret for text inputs, in the spirit of Monkeytype. The native caret
// is hidden (caret-color: transparent) and a positioned bar eases to the text
// cursor's x offset, so typing reads as a glide rather than a jump. Used by the
// main search input and the command-mode input bars.
//
// The x is measured with a hidden mirror span that copies the input's font
// metrics and holds the text up to the caret: its width is the caret offset.
// A mirror (not canvas measureText) keeps it exact under letter-spacing, which
// the Linux build adds and canvas ignores.
//
// Each input gets its own caret + mirror, keyed in `attached`. Only the focused
// input's caret is visible, so several can be wired without them clashing.

const attached = new Map();

// While the caret is moving it stays solid; it resumes blinking once typing
// pauses. Matches the Monkeytype feel.
const BLINK_RESUME_MS = 400;

/**
 * Wire a smooth caret onto `input`. No-op if it has none of the recognised
 * bar containers, or if it is already attached.
 */
export function attach(input) {
    if (!input || attached.has(input)) return;
    const bar = input.closest('.search-bar, .cmd-input-bar');
    if (!bar) return;

    const caret = document.createElement('span');
    caret.className = 'smooth-caret';
    bar.appendChild(caret);

    const mirror = document.createElement('span');
    mirror.className = 'smooth-caret-mirror';
    mirror.setAttribute('aria-hidden', 'true');
    bar.appendChild(mirror);

    const state = { caret, mirror, idleTimer: null };
    attached.set(input, state);

    const reposition = () => place(input, state);
    // Any of these can move the cursor. keydown updates on the next frame so the
    // value/selection reflect the just-pressed key before we measure.
    for (const ev of ['input', 'keyup', 'click', 'select']) {
        input.addEventListener(ev, reposition);
    }
    input.addEventListener('keydown', () => requestAnimationFrame(reposition));
    input.addEventListener('focus', () => {
        caret.classList.add('is-on');
        reposition();
    });
    input.addEventListener('blur', () => caret.classList.remove('is-on'));

    if (document.activeElement === input) caret.classList.add('is-on');
    reposition();
}

/**
 * Re-sync an input's caret after a programmatic value/selection change (which
 * fires no `input`). No-op for an input that was never attached.
 */
export function refresh(input) {
    const state = attached.get(input);
    if (state) place(input, state);
}

// Reposition the caret to the current cursor offset for one attached input.
function place(input, state) {
    const { caret, mirror } = state;

    const pos = input.selectionEnd ?? input.value.length;
    mirror.textContent = input.value.slice(0, pos);

    // Copy the live font metrics so the mirror renders text at the same width
    // (font-size tracks the zoom setting, so read it fresh).
    const s = getComputedStyle(input);
    mirror.style.fontFamily = s.fontFamily;
    mirror.style.fontSize = s.fontSize;
    mirror.style.fontWeight = s.fontWeight;
    mirror.style.fontStyle = s.fontStyle;
    mirror.style.letterSpacing = s.letterSpacing;

    const x = input.offsetLeft + mirror.offsetWidth - input.scrollLeft;
    caret.style.transform = `translateX(${x}px)`;

    // Solid while moving, blink again after a beat of stillness.
    caret.classList.add('is-typing');
    clearTimeout(state.idleTimer);
    state.idleTimer = setTimeout(() => caret.classList.remove('is-typing'), BLINK_RESUME_MS);
}
