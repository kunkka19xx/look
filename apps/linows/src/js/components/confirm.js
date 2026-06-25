let confirmEl = null;
let iconEl = null;
let titleEl = null;
let detailEl = null;
let yesBtn = null;
let noBtn = null;
let resolveFn = null;
let active = false;

export function init(el) {
  confirmEl = el;
  iconEl = el.querySelector('.confirm-icon');
  titleEl = el.querySelector('.confirm-title');
  detailEl = el.querySelector('.confirm-detail');
  yesBtn = el.querySelector('.confirm-yes');
  noBtn = el.querySelector('.confirm-no');
  yesBtn.addEventListener('click', () => settle(true));
  noBtn.addEventListener('click', () => settle(false));
}

export function isActive() {
  return active;
}

export function ask({ title, detail, yesLabel = 'Y / Yes', noLabel = 'N / No', icon = null }) {
  if (!confirmEl) {
    return Promise.resolve(false);
  }
  if (active) settle(false);

  iconEl.innerHTML = icon || '';
  iconEl.hidden = !icon;
  titleEl.textContent = title;
  detailEl.textContent = detail || '';
  yesBtn.textContent = yesLabel;
  noBtn.textContent = noLabel;
  confirmEl.hidden = false;
  document.body.classList.add('confirm-active');
  active = true;
  return new Promise((resolve) => {
    resolveFn = resolve;
  });
}

export function confirm() {
  settle(true);
}

export function cancel() {
  settle(false);
}

function settle(result) {
  if (!active) return;
  active = false;
  confirmEl.hidden = true;
  document.body.classList.remove('confirm-active');
  const r = resolveFn;
  resolveFn = null;
  if (r) r(result);
}
