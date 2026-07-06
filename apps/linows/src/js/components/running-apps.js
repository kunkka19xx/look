import { listRunningApps, getIcon, activateRunningApp } from '../ipc.js';
import { appWindow, settings } from '../icons.js';

// Ergonomic badge keys - easy-to-reach keys first (1,2,3,9,8,4,7,6,5).
// We assign in this order then sort ascending for visual display.
const EASINESS_ORDER = [1, 2, 3, 9, 8, 4, 7, 6, 5];
const MAX_ITEMS = 9;

const iconCache = new Map();

let container = null;
let apps = []; // sorted alphabetically, max 9
let enabled = true;
let suspended = false; // temporarily hidden (e.g. command mode, settings)

export function init(containerEl) {
  container = containerEl;
}

export function setEnabled(on) {
  enabled = on;
  if (container) container.hidden = !on || suspended;
}

export function isEnabled() {
  return enabled;
}

/** Temporarily hide the strip without changing the user's enabled preference.
 *  Used when entering non-main screens (command mode, settings, help). */
export function setSuspended(on) {
  suspended = on;
  if (container && on) container.hidden = true;
}

/** Refresh the running apps list from backend. */
export async function refresh() {
  if (!enabled || suspended || !container) return;

  try {
    const procs = await listRunningApps();
    // Re-check after await: user may have entered a suspended mode
    // (command, translate) while the IPC call was in flight.
    if (!enabled || suspended || !container) return;
    // Already sorted alphabetically by backend, take first 9
    apps = procs.slice(0, MAX_ITEMS);
    render();
  } catch (err) {
    console.error('[running-apps] refresh failed:', err);
  }
}

/** Activate a running app by its badge key (1-9). Returns true if handled. */
export function activateByKey(key) {
  if (!enabled || suspended || apps.length === 0) return false;

  const total = apps.length;
  const keys = badgeKeys(total);
  const visualIdx = keys.indexOf(key);
  if (visualIdx < 0) return false;

  const app = apps[visualIdx];
  activateRunningApp(app.pid, app.desktop_id, app.exec).catch((err) => {
    console.error('[running-apps] activate failed:', err);
  });
  return true;
}

// --- Badge key assignment (mirrors macOS AppConstants) ---

function badgeKeys(total) {
  if (total <= 0) return [];
  const picked = EASINESS_ORDER.slice(0, Math.min(total, MAX_ITEMS));
  return picked.slice().sort((a, b) => a - b);
}

// --- Rendering ---

function render() {
  container.innerHTML = '';
  if (apps.length === 0) {
    container.hidden = true;
    return;
  }
  container.hidden = false;

  const keys = badgeKeys(apps.length);

  apps.forEach((app, i) => {
    const item = document.createElement('div');
    item.className = 'running-app-item';
    item.title = app.name;

    // Icon
    const icon = document.createElement('div');
    icon.className = 'running-app-icon';
    // Letter placeholder
    icon.textContent = (app.name || '?')[0].toUpperCase();
    item.appendChild(icon);

    // Badge
    const badge = document.createElement('span');
    badge.className = 'running-app-badge';
    badge.textContent = keys[i];
    item.appendChild(badge);

    // Click to activate
    item.addEventListener('click', (e) => {
      e.stopPropagation();
      activateRunningApp(app.pid, app.desktop_id, app.exec).catch(() => {});
    });

    container.appendChild(item);

    // UWP windows (Settings, …) are addressed by HWND and have no resolvable
    // file icon - the shell returns a generic "earth" globe. Use a Lucide app
    // glyph instead. Everything else resolves its real icon async.
    if (app.desktop_id && app.desktop_id.startsWith('hwnd:')) {
      icon.textContent = '';
      // Settings → gear; other UWP apps (Calculator, Photos, …) → app-window.
      icon.innerHTML = /settings/i.test(app.name) ? settings : appWindow;
    } else if (app.desktop_id) {
      loadAppIcon(icon, app.desktop_id);
    }
  });
}

function loadAppIcon(iconEl, desktopId) {
  if (iconCache.has(desktopId)) {
    const dataUrl = iconCache.get(desktopId);
    if (dataUrl) applyIcon(iconEl, dataUrl);
    return;
  }

  // desktop_id is "app:/path/to/foo.desktop" - pass kind="app", path from id
  const path = desktopId.startsWith('app:') ? desktopId.slice(4) : desktopId;
  getIcon('app', path, desktopId)
    .then((result) => {
      const dataUrl = result?.data_url || null;
      iconCache.set(desktopId, dataUrl);
      if (dataUrl) applyIcon(iconEl, dataUrl);
    })
    .catch(() => {
      iconCache.set(desktopId, null);
    });
}

function applyIcon(iconEl, dataUrl) {
  const img = document.createElement('img');
  img.src = dataUrl;
  img.alt = '';
  img.draggable = false;
  iconEl.textContent = '';
  iconEl.appendChild(img);
}
