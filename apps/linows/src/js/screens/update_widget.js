// Notify-only update widget mounted in Settings ("About" footer) and Help.
// We never download or replace the binary — Linux/Windows distros vary too
// much to bundle a single upgrade command, so the banner links to the
// release notes and the README install section instead.
//
// The check itself runs in the webview via `fetch()` (no Rust HTTP/TLS
// dep); cross-origin works because `tauri.conf.json` has `csp: null`.
// Dismissed versions persist in `localStorage`.
import { getLookappVersion, openPath, isDevBuild } from '../ipc.js';
import * as platform from '../platform.js';

const RELEASES_API_URL = 'https://api.github.com/repos/kunkka19xx/look/releases/latest';
const FALLBACK_RELEASES_URL = 'https://github.com/kunkka19xx/look/releases/latest';
const DISMISSED_VERSION_KEY = 'look.update.dismissedVersion';
const HTTP_TIMEOUT_MS = 15_000;

// Linux/Windows each have several install paths (deb / AppImage / NixOS /
// Nix profile / NSIS / install scripts); sending the user to the README
// section lets them copy the command that matches their setup.
const INSTALL_HINT_URLS = {
  linux: 'https://github.com/kunkka19xx/look#linux',
  windows: 'https://github.com/kunkka19xx/look#windows',
};

// `container -> label`: Settings mounts with "About", Help mounts bare.
const widgets = new Map();
const state = {
  currentVersion: '',
  isDev: false,
  available: null,
  status: '',
  isChecking: false,
};

export async function mountUpdateWidget(container, { label = '' } = {}) {
  if (!container) return;
  container.classList.add('update-widget');
  widgets.set(container, label);
  render(container);
  if (!state.currentVersion) {
    try {
      [state.currentVersion, state.isDev] = await Promise.all([
        getLookappVersion(),
        isDevBuild(),
      ]);
    } catch {
      state.currentVersion = '';
      state.isDev = false;
    }
    renderAll();
  }
}

async function handleCheck() {
  if (state.isChecking) return;
  state.isChecking = true;
  state.status = 'Checking…';
  renderAll();
  try {
    const result = await performCheck(state.currentVersion, true);
    state.available = result.available;
    state.status = result.status;
  } catch {
    state.status = "Couldn't check for updates";
    state.available = null;
  } finally {
    state.isChecking = false;
    renderAll();
  }
}

function handleDismiss() {
  if (!state.available) return;
  try {
    localStorage.setItem(DISMISSED_VERSION_KEY, state.available.version);
  } catch {
    // Private mode or quota errors — in-memory dismissal still works.
  }
  state.available = null;
  state.status = '';
  renderAll();
}

function handleNotes() {
  if (!state.available) return;
  openPath(state.available.release_url, 'browser', '');
}

function handleInstallHint() {
  const url = INSTALL_HINT_URLS[platform.os()];
  if (!url) return;
  openPath(url, 'browser', '');
}

async function performCheck(currentVersion, force) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), HTTP_TIMEOUT_MS);
  let response;
  try {
    response = await fetch(RELEASES_API_URL, {
      headers: { Accept: 'application/vnd.github+json' },
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timer);
  }
  if (!response.ok) {
    return { available: null, status: "Couldn't check for updates" };
  }
  const json = await response.json();
  const tag = typeof json?.tag_name === 'string' ? json.tag_name : null;
  if (!tag) {
    return { available: null, status: "Couldn't check for updates" };
  }
  if (json.draft === true || json.prerelease === true) {
    return latestStatus(currentVersion);
  }
  const latest = normalizeVersion(tag);
  if (!isVersionNewer(latest, normalizeVersion(currentVersion))) {
    return latestStatus(currentVersion);
  }
  // Allow-list GitHub URLs — a compromised response shouldn't hand
  // `xdg-open` an arbitrary scheme.
  const rawUrl = typeof json.html_url === 'string' ? json.html_url : '';
  const releaseUrl = rawUrl.startsWith('https://github.com/') ? rawUrl : FALLBACK_RELEASES_URL;
  // A manual check ("force") overrides a prior dismissal of the same version.
  if (!force && safeGetItem(DISMISSED_VERSION_KEY) === latest) {
    return { available: null, status: '' };
  }
  return {
    available: { version: latest, release_url: releaseUrl },
    status: `Update available: Look ${latest}`,
  };
}

function latestStatus(currentVersion) {
  const label = currentVersion || 'unknown';
  return { available: null, status: `You're on the latest version (${label})` };
}

function safeGetItem(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function normalizeVersion(raw) {
  const trimmed = String(raw).trim();
  return trimmed.startsWith('v') || trimmed.startsWith('V') ? trimmed.slice(1) : trimmed;
}

// Numeric dotted compare: "1.10.0" > "1.9.0". Non-numeric segments → 0.
function isVersionNewer(lhs, rhs) {
  const a = components(lhs);
  const b = components(rhs);
  const count = Math.max(a.length, b.length);
  for (let i = 0; i < count; i++) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (x !== y) return x > y;
  }
  return false;
}

function components(version) {
  return String(version).split('.').map((part) => parseInt(part, 10) || 0);
}

function renderAll() {
  widgets.forEach((_label, container) => render(container));
}

function render(container) {
  const label = widgets.get(container) || '';
  const { currentVersion, isDev, available, status, isChecking } = state;
  const devSuffix = isDev ? ' — dev' : '';
  const versionLabel = currentVersion
    ? `Look ${escapeHtml(currentVersion)}${devSuffix}`
    : 'Look …';
  // Suppress the status text once a real update has surfaced — the banner below
  // says the same thing more loudly.
  const showStatus = status && !available;

  let html = '';
  if (label) {
    html += `<div class="update-section-label">${escapeHtml(label)}</div>`;
  }
  html += `
    <div class="update-row">
      <span class="update-version">${versionLabel}</span>
      ${showStatus ? `<span class="update-status">${escapeHtml(status)}</span>` : ''}
      <button class="update-pill" type="button" data-action="check"${isChecking ? ' disabled' : ''}>
        ${isChecking ? 'Checking…' : 'Check for Updates'}
      </button>
    </div>
  `;

  if (available) {
    html += `
      <div class="update-banner">
        <span class="update-banner-text">Update available: Look ${escapeHtml(available.version)}</span>
        <button class="update-pill" type="button" data-action="notes">Release Notes</button>
        <button class="update-pill update-pill-muted" type="button" data-action="dismiss">Dismiss</button>
      </div>
    `;
    const os = platform.os();
    if (INSTALL_HINT_URLS[os]) {
      const osLabel = os === 'windows' ? 'Windows' : 'Linux';
      html += `
        <div class="update-hint">
          Update on ${osLabel}: <a class="update-hint-link" href="#" data-action="install-hint">see install instructions</a>
        </div>
      `;
    }
  }

  container.innerHTML = html;
  container.querySelector('[data-action="check"]')?.addEventListener('click', handleCheck);
  container.querySelector('[data-action="notes"]')?.addEventListener('click', handleNotes);
  container.querySelector('[data-action="dismiss"]')?.addEventListener('click', handleDismiss);
  container.querySelector('[data-action="install-hint"]')?.addEventListener('click', (e) => {
    e.preventDefault();
    handleInstallHint();
  });
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  }[c]));
}
