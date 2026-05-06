let bannerEl = null;
let textEl = null;
let hideTimer = null;

export function init(el) {
  bannerEl = el;
  textEl = el.querySelector('.banner-text');
}

export function show(message, style = 'info', duration = 1.5) {
  if (!bannerEl || !textEl) return;

  clearTimeout(hideTimer);

  textEl.textContent = message;
  bannerEl.className = `banner banner-${style}`;
  bannerEl.hidden = false;

  hideTimer = setTimeout(() => {
    bannerEl.hidden = true;
  }, duration * 1000);
}
