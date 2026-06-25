import { search as ipcSearch, getClipboardHistory } from './ipc.js';

const DEBOUNCE_MS = 70;
const MIN_QUICK_FOLDER_PREFIX = 2;
const SEARCH_LIMIT = 40;
const CLIPBOARD_TITLE_MAX_CHARS = 80;
let debounceTimer = null;
let onResultsCallback = null;
let homeDir = null;
let clipboardMode = false;
let translateMode = false;
let recentMode = false;

/** Resolved by the backend (Windows: SHGetKnownFolderPath; *nix: $HOME/<name>).
 *  Empty until setQuickFolders is called at boot. */
let quickFolders = [];

export function setOnResults(callback) {
  onResultsCallback = callback;
}

export function setHomeDir(home) {
  homeDir = home;
}

export function setQuickFolders(list) {
  quickFolders = Array.isArray(list) ? list : [];
}

export function isClipboardMode() {
  return clipboardMode;
}

export function isTranslateMode() {
  return translateMode;
}

export function isRecentMode() {
  return recentMode;
}

export function getTranslateText() {
  return translateMode ? _translateText : '';
}

let _translateText = '';

export function handleQueryInput(query) {
  clearTimeout(debounceTimer);

  if (query.startsWith('t"')) {
    translateMode = true;
    clipboardMode = false;
    recentMode = false;
    _translateText = query.slice(2).trim();
    // Translation is triggered on Enter, not on typing
    // Show empty results with hint
    if (onResultsCallback) onResultsCallback([], query);
    return;
  }

  translateMode = false;

  if (query.startsWith('c"')) {
    clipboardMode = true;
    recentMode = false;
    const filter = query.slice(2);
    debounceTimer = setTimeout(() => performClipboardSearch(filter), DEBOUNCE_MS);
    return;
  }

  clipboardMode = false;

  if (query.startsWith('rc"')) {
    recentMode = true;
    debounceTimer = setTimeout(() => performSearch(query), DEBOUNCE_MS);
    return;
  }

  recentMode = false;

  if (query.trim() === '') {
    performSearch('');
    return;
  }

  debounceTimer = setTimeout(() => performSearch(query), DEBOUNCE_MS);
}

async function performSearch(query) {
  try {
    const payload = await ipcSearch(query, SEARCH_LIMIT);
    const results = recentMode ? payload.results : prependQuickFolders(payload.results, query);
    if (onResultsCallback) {
      onResultsCallback(results, query);
    }
  } catch (err) {
    console.error('Search failed:', err);
    if (onResultsCallback) {
      onResultsCallback([], query);
    }
  }
}

function formatShortDate(timestamp) {
  const d = new Date(timestamp * 1000);
  const yr = String(d.getFullYear()).slice(2);
  const mo = d.getMonth() + 1;
  const day = d.getDate();
  const hr = d.getHours();
  const min = String(d.getMinutes()).padStart(2, '0');
  const ampm = hr >= 12 ? 'PM' : 'AM';
  const h12 = hr % 12 || 12;
  return `${mo}/${day}/${yr}, ${h12}:${min} ${ampm}`;
}

function formatMediumDate(timestamp) {
  const d = new Date(timestamp * 1000);
  const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
  const sec = String(d.getSeconds()).padStart(2, '0');
  const min = String(d.getMinutes()).padStart(2, '0');
  const hr = d.getHours();
  const ampm = hr >= 12 ? 'PM' : 'AM';
  const h12 = hr % 12 || 12;
  return `${months[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()} at ${h12}:${min}:${sec} ${ampm}`;
}

async function performClipboardSearch(filter) {
  try {
    const entries = await getClipboardHistory(filter);
    const results = entries.map((e, i) => {
      // Title: first 80 chars, newlines → spaces
      const title = e.text.replace(/\n/g, '  ').slice(0, CLIPBOARD_TITLE_MAX_CHARS) || '(empty)';
      const shortDate = formatShortDate(e.timestamp);
      const subtitle = `Clipboard  \u2022  ${e.char_count} chars  \u2022  ${e.line_count} lines  \u2022  ${shortDate}`;
      return {
        id: `clip:${i}`,
        kind: 'clipboard',
        title,
        subtitle,
        path: 'clipboard://history',
        score: 0,
        clipIndex: i,
        clipText: e.text,
        clipTimestamp: e.timestamp,
        clipCharCount: e.char_count,
        clipLineCount: e.line_count,
        clipDateShort: shortDate,
        clipDateMedium: formatMediumDate(e.timestamp),
      };
    });
    if (onResultsCallback) onResultsCallback(results, `c"${filter}`);
  } catch (err) {
    console.error('Clipboard search failed:', err);
    if (onResultsCallback) onResultsCallback([], `c"${filter}`);
  }
}

export { formatMediumDate };

function prependQuickFolders(results, query) {
  if (quickFolders.length === 0) return results;
  const q = query.toLowerCase().trim();
  if (q.length < MIN_QUICK_FOLDER_PREFIX) return results;

  const matched = [];
  for (const folder of quickFolders) {
    if (!folder.title.toLowerCase().startsWith(q)) continue;
    if (results.some((r) => r.path === folder.path)) continue;
    const isTrash = folder.title === 'Trash';
    matched.push({
      id: `quickfolder:${folder.title.toLowerCase()}`,
      kind: 'folder',
      title: folder.title,
      subtitle: isTrash ? 'Pinned · Ctrl+D to empty' : 'Pinned home folder',
      path: folder.path,
      score: 999999,
    });
  }

  return [...matched, ...results];
}
