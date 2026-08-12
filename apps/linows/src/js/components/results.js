import { getIcon } from '../ipc.js';
import {
    clipboard as clipboardIcon,
    check as checkIcon,
    cpu as cpuIcon,
    appIcon,
    fileIcon,
    folderIcon,
    settingIcon,
    historyLg,
} from '../icons.js';
import { getSettingsIcon as getWindowsSettingsIcon } from '../settings-icons/windows.js';
import { classifyResultId } from '../catalog.js';
import { prefersReducedMotion } from '../platform.js';
import * as layout from '../layout.js';

// LRU-bounded icon cache (cacheKey -> data URL | null). A plain Map keeps
// insertion order, so re-inserting on a hit marks it most-recently-used and the
// oldest key is always keys().next(). Bounds heap growth while browsing many
// icons; evicted entries re-resolve instantly from the backend's own cache.
const ICON_CACHE_MAX = 256;
// Marks the row (and pill) taking the selection, for the one-shot zoom.
const GAIN_CLASS = 'is-gaining';
const iconCache = new Map();
const pickedMap = new Map(); // key → result

function iconCacheGet(key) {
    if (!iconCache.has(key)) return undefined;
    const val = iconCache.get(key);
    iconCache.delete(key);
    iconCache.set(key, val);
    return val;
}

function iconCacheSet(key, val) {
    iconCache.delete(key);
    iconCache.set(key, val);
    if (iconCache.size > ICON_CACHE_MAX) {
        iconCache.delete(iconCache.keys().next().value);
    }
}

let currentResults = [];
let selectedIndex = -1;
let container = null;
// One pill behind the rows that glides to the selected row instead of each row
// crossfading its own background. Lives inside the scrolling list so it tracks
// scroll for free; wiped with the rows on every render, rebuilt lazily.
let selectionPill = null;
// Honor prefers-reduced-motion for the scroll too: the pill glide is killed in
// CSS, but scrollIntoView's smooth behavior bypasses CSS, so gate it here.
let onSelectionChange = null;
let onPickChange = null;
let emptyState = { mode: 'default' };
// True once the user moves the cursor (arrows/Tab/click) within the current
// query. Gates selection preservation in render(): a cursor the user parked
// somewhere survives re-renders, but an auto-selected row does not - search
// publishes progressively (engine, then URL rows, then web suggestions) and
// a row that painted first may end up mid/bottom of the final order. Without
// the gate the cursor follows it there instead of resting on the top result.
let userNavigated = false;
let lastRenderQuery = null;

export function init(containerEl) {
    container = containerEl;
}

export function setOnSelectionChange(callback) {
    onSelectionChange = callback;
}

export function setOnPickChange(callback) {
    onPickChange = callback;
}

export function setEmptyState(state) {
    emptyState = state || { mode: 'default' };
    if (currentResults.length === 0 && container) {
        container.innerHTML = renderEmptyState();
    }
}

function renderEmptyState() {
    // Left half of the clipboard empty state; the preview column shows the
    // "How to use" half (macOS ClipboardEmptyInfoView / ClipboardEmptyHelpView).
    if (emptyState.mode === 'clipboard') {
        return `
      <div class="empty-state empty-state-rich">
        <div class="empty-state-icon">${clipboardIcon}</div>
        <div class="empty-state-title">Clipboard History</div>
        <div class="empty-state-body">No clipboard items yet</div>
        <div class="empty-state-help">Copy any text, then search with <kbd>c"word</kbd> to find it here.</div>
      </div>`;
    }
    if (emptyState.mode === 'recent') {
        return `
      <div class="empty-state empty-state-rich">
        <div class="empty-state-icon">${historyLg}</div>
        <div class="empty-state-title">Recent files &amp; folders</div>
        <div class="empty-state-body">Nothing recent yet</div>
        <div class="empty-state-help">Open files/folders through Look, or download/create some - newest activity shows here. Type <kbd>rc"word</kbd> to filter.</div>
      </div>`;
    }
    // AI two-col mode: the right column hosts web suggestions, which routinely
    // return empty (DDG /ac/ rate-limits, transient curl failures). Showing
    // a stark "No results" there reads as broken when the user can see the
    // answer card on the left is working fine. Render nothing instead.
    if (emptyState.mode === 'ai-suggestion') {
        return '';
    }
    return '<div class="empty-state">No results</div>';
}

export function render(results, query = null) {
    // A new query invalidates any manual cursor position.
    if (query !== lastRenderQuery) {
        lastRenderQuery = query;
        userNavigated = false;
    }

    // Preserve the selected row across re-renders, but only when the user put
    // the cursor there: a file-watcher index refresh fires `index-ready`, which
    // re-runs the current query and re-publishes the (often identical) result
    // set. Without this, the cursor snaps back to row 0 mid-scroll a couple
    // seconds after the user picks another row. Auto-selected rows are exempt
    // (see userNavigated above) so the cursor lands on row 0 once the full
    // result order settles.
    const prevSelectedId =
        userNavigated && selectedIndex >= 0 && selectedIndex < currentResults.length
            ? currentResults[selectedIndex].id
            : null;

    currentResults = results;
    container.innerHTML = '';

    if (results.length === 0) {
        container.innerHTML = renderEmptyState();
        selectedIndex = -1;
        return;
    }

    results.forEach((result, index) => {
        const row = createRow(result, index);
        container.appendChild(row);
    });

    let nextIndex = 0;
    if (prevSelectedId != null) {
        const idx = results.findIndex((r) => r.id === prevSelectedId);
        if (idx >= 0) nextIndex = idx;
    }

    if (layout.isEmptyQuery(query) && layout.hidesResultsForEmptyQuery()) {
        // No rows on screen. A seeded selection here is one the user cannot
        // see, and Enter / Ctrl+D / Ctrl+Shift+H would still act on it.
        selectedIndex = -1;
        return;
    }

    select(nextIndex);
}

export function getSelected() {
    if (selectedIndex >= 0 && selectedIndex < currentResults.length) {
        return currentResults[selectedIndex];
    }
    return null;
}

export function getSelectedIndex() {
    return selectedIndex;
}

export function selectNext() {
    if (currentResults.length === 0) return;
    userNavigated = true;
    select((selectedIndex + 1) % currentResults.length, true);
}

export function selectPrev() {
    if (currentResults.length === 0) return;
    userNavigated = true;
    select((selectedIndex - 1 + currentResults.length) % currentResults.length, true);
}

// `glide` slides the pill between rows; it's on for keyboard moves and off for a
// fresh render, click, or data refresh, where the pill should just be there.
export function select(index, glide = false) {
    const prev = container.querySelector('.result-row.selected');
    if (prev) prev.classList.remove('selected', GAIN_CLASS);

    selectedIndex = index;

    const rows = container.querySelectorAll('.result-row');
    const row = rows[index];
    if (row) {
        row.classList.add('selected');
        placeSelectionPill(row, glide);
        // Keyboard moves only: a re-render per keystroke would pulse row 0.
        if (glide) playGain(row);
        row.scrollIntoView({
            block: 'nearest',
            behavior: prefersReducedMotion() ? 'auto' : 'smooth',
        });
    }

    if (onSelectionChange) {
        onSelectionChange(getSelected());
    }
}

// The pill is absolutely positioned in the list's content box, so its top:0 and
// a row's offsetTop share an origin: translateY + height overlay it exactly. Its
// left/right insets come from CSS (they match the row margin), so only the
// vertical geometry is set here.
function placeSelectionPill(row, glide) {
    if (!selectionPill || selectionPill.parentNode !== container) {
        selectionPill = document.createElement('div');
        selectionPill.className = 'results-selection';
        container.prepend(selectionPill);
    }
    if (!glide) selectionPill.classList.add('is-instant');
    selectionPill.style.transform = `translateY(${row.offsetTop}px)`;
    selectionPill.style.height = `${row.offsetHeight}px`;
    selectionPill.classList.add('is-visible');
    if (!glide) {
        // Commit the jump this frame, then restore gliding for later moves.
        void selectionPill.offsetWidth;
        selectionPill.classList.remove('is-instant');
    }
}

// One-shot zoom on the row taking the selection, and the pill under it. Row-
// local by design (see motion.css); restarting needs a forced reflow.
function playGain(row) {
    for (const el of [row, selectionPill]) {
        if (!el) continue;
        el.classList.remove(GAIN_CLASS);
        void el.offsetWidth;
        el.classList.add(GAIN_CLASS);
    }
}

// --- Pick management ---

function pickKey(item) {
    return `${item.kind}|${item.path}`;
}

export function togglePick(item) {
    if (!item) return;
    const key = pickKey(item);
    if (pickedMap.has(key)) {
        pickedMap.delete(key);
    } else {
        pickedMap.set(key, item);
    }
    updatePickedIndicators();
    if (onPickChange) onPickChange(getPickedItems());
}

export function removePick(key) {
    pickedMap.delete(key);
    updatePickedIndicators();
    if (onPickChange) onPickChange(getPickedItems());
}

export function clearPicks() {
    pickedMap.clear();
    updatePickedIndicators();
    if (onPickChange) onPickChange(getPickedItems());
}

export function isPicked(item) {
    return pickedMap.has(pickKey(item));
}

export function getPickedItems() {
    return [...pickedMap.entries()].map(([key, item]) => ({ key, ...item }));
}

export function hasPickedItems() {
    return pickedMap.size > 0;
}

function updatePickedIndicators() {
    const rows = container.querySelectorAll('.result-row');
    rows.forEach((row, i) => {
        const result = currentResults[i];
        if (!result) return;
        const check = row.querySelector('.pick-check');
        if (pickedMap.has(pickKey(result))) {
            if (!check) {
                const el = document.createElement('div');
                el.className = 'pick-check';
                el.innerHTML = checkIcon;
                row.appendChild(el);
            }
        } else if (check) {
            check.remove();
        }
    });
}

// --- Row creation ---

// Settings entries pack their full alias list into `subtitle` so the engine
// can fuzzy-match on keywords ("wifi", "ssid", "captions" …). That's great
// for search but renders as a long noisy line ("Windows Settings settings
// wifi wireless network ssid"). Trim the alias tail at render time - engine
// keeps the full string for scoring.
const SETTINGS_SUBTITLE_PREFIXES = ['Windows Settings', 'System Settings'];

function displaySubtitle(result) {
    if (result.kind === 'clipboard') return result.subtitle;
    if (result.subtitle) {
        for (const prefix of SETTINGS_SUBTITLE_PREFIXES) {
            if (result.subtitle.startsWith(prefix + ' ')) return prefix;
        }
        return result.subtitle;
    }
    if (result.kind === 'file' || result.kind === 'folder') return result.path;
    const kindLabels = { app: 'App', setting: 'Setting' };
    return kindLabels[result.kind] || result.kind;
}

function createRow(result, index) {
    const row = document.createElement('div');
    row.className = 'result-row';
    row.dataset.index = index;
    // Web-suggestion rows live in a narrow 320 px column when the AI card is
    // active; let their title wrap to multiple lines instead of truncating
    // (matches macOS WebSuggestionPreviewView's 3-line title cap).
    if (classifyResultId(result.id)?.kind === 'webSuggestion') {
        row.classList.add('result-row-web-suggest');
    }

    // Icon (kind-based SVG fallback, async-load real icon)
    const icon = document.createElement('div');
    icon.className = 'result-icon';
    const isLinuxSettings =
        result.path?.startsWith('settings://') ||
        result.subtitle?.toLowerCase().startsWith('settings');
    // Windows ms-settings: panels share one icon at the OS level (the gear) - we
    // map each panel to a category-specific Lucide glyph via the catalog so the
    // list scans visually. Returns null if the path isn't an ms-settings URI.
    const windowsSettingsSvg = getWindowsSettingsIcon(result.path);
    const fallbacks = {
        file: fileIcon,
        folder: folderIcon,
        setting: settingIcon,
        clipboard: clipboardIcon,
        process: cpuIcon,
    };
    // Synthetic discovery rows (prefix/command menus) ship their own glyph in
    // result.iconSvg so the list scans visually; everything else falls back to
    // the kind-based stub until the backend icon fetch resolves.
    icon.innerHTML =
        result.iconSvg ||
        windowsSettingsSvg ||
        (isLinuxSettings ? settingIcon : fallbacks[result.kind] || appIcon);
    icon.style.background = 'var(--control-fill)';
    icon.style.color = 'var(--font-secondary)';
    row.appendChild(icon);

    // Skip backend icon fetch for ms-settings entries - the Shell PNG would just
    // be the generic gear and would clobber our category-specific glyph. Same
    // applies to synthetic discovery rows whose `path` is empty.
    if (result.kind === 'process') {
        // App-backed processes wear their app icon (loaded as an app path);
        // the rest keep the generic process glyph from the fallback above.
        if (result.iconPath) {
            loadIcon(icon, 'app', result.iconPath, result.id);
        }
    } else if (
        result.kind !== 'clipboard' &&
        !windowsSettingsSvg &&
        !result.iconSvg &&
        result.path
    ) {
        loadIcon(icon, result.kind, result.path, result.id);
    }

    // Text content
    const text = document.createElement('div');
    text.className = 'result-text';

    const title = document.createElement('div');
    title.className = 'result-title';
    title.textContent = result.title;
    text.appendChild(title);

    const subtitle = document.createElement('div');
    subtitle.className = 'result-path';
    subtitle.textContent = displaySubtitle(result);
    text.appendChild(subtitle);

    row.appendChild(text);

    // Picked indicator
    if (pickedMap.has(pickKey(result))) {
        const el = document.createElement('div');
        el.className = 'pick-check';
        el.innerHTML = checkIcon;
        row.appendChild(el);
    }

    row.addEventListener('click', () => {
        userNavigated = true;
        select(index);
        row.dispatchEvent(new CustomEvent('result-activate', { bubbles: true }));
    });

    return row;
}

function loadIcon(iconEl, kind, path, id) {
    const cacheKey = `${kind}:${path}`;

    const cached = iconCacheGet(cacheKey);
    if (cached !== undefined) {
        // null = known to have no icon; skip the re-fetch either way.
        if (cached) applyIcon(iconEl, cached);
        return;
    }

    getIcon(kind, path, id)
        .then((result) => {
            const dataUrl = result?.data_url || null;
            iconCacheSet(cacheKey, dataUrl);
            if (dataUrl) {
                applyIcon(iconEl, dataUrl);
            }
        })
        .catch(() => {
            iconCacheSet(cacheKey, null);
        });
}

function applyIcon(iconEl, dataUrl) {
    const img = document.createElement('img');
    img.src = dataUrl;
    img.alt = '';
    iconEl.textContent = '';
    iconEl.style.background = 'none';
    iconEl.appendChild(img);
}
