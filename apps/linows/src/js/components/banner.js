let bannerEl = null;
let textEl = null;
let dismissEl = null;
let hideTimer = null;
// Two layers share the banner slot: transient toasts (auto-hide) and a
// sticky notice (stays until dismissed). An active toast wins; when it
// expires the sticky notice resurfaces instead of the banner hiding.
let toast = null;
let sticky = null;

export function init(el) {
    bannerEl = el;
    textEl = el.querySelector('.banner-text');
    dismissEl = el.querySelector('.banner-dismiss');
    dismissEl.addEventListener('click', () => {
        const onDismiss = sticky?.onDismiss;
        sticky = null;
        render();
        if (onDismiss) onDismiss();
    });
}

export function show(message, style = 'info', duration = 1.5) {
    if (!bannerEl) return;

    clearTimeout(hideTimer);
    toast = { message, style };
    render();

    hideTimer = setTimeout(() => {
        toast = null;
        render();
    }, duration * 1000);
}

// Sticky notice with a dismiss button. Pass a falsy message to clear it.
export function showSticky(message, style = 'warning', onDismiss = null) {
    if (!bannerEl) return;
    sticky = message ? { message, style, onDismiss } : null;
    render();
}

function render() {
    const active = toast || sticky;
    if (!active) {
        bannerEl.hidden = true;
        return;
    }
    textEl.textContent = active.message;
    bannerEl.className = `banner banner-${active.style}`;
    dismissEl.hidden = active !== sticky;
    bannerEl.hidden = false;
}
