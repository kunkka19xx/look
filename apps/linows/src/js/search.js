import { search as ipcSearch, getClipboardHistory, webSuggestions as ipcWebSuggestions } from './ipc.js';
import {
  isPrefixSuggestionQuery, isCommandSuggestionQuery, isPrefixedQuery,
  prefixSuggestionResults, commandSuggestionResults, webSuggestionResults,
} from './catalog.js';

const DEBOUNCE_MS = 70;
const MIN_QUICK_FOLDER_PREFIX = 2;
const SEARCH_LIMIT = 40;
const WEB_SUGGESTIONS_LIMIT = 6;
const MIN_WEB_SUGGESTION_QUERY_LENGTH = 2;
const CLIPBOARD_TITLE_MAX_CHARS = 80;
let debounceTimer = null;
let webSuggestionTimer = null;
let onResultsCallback = null;
let clipboardMode = false;
let translateMode = false;
let recentMode = false;
let prefixHintMode = false;
let commandHintMode = false;
let aiEnabled = true;

// Per-query state. Each leg of the parallel fetch (engine + web suggestions)
// writes into its slot; publish() merges them. Version counter discards
// in-flight responses whose query has been superseded. lastQueryString lets
// us tell same-query re-runs (e.g. window toggle) apart from a genuine new
// query, so we don't wipe a still-valid suggestion list during a re-fetch.
// webInFlight gates publish so we don't paint a "No results" empty state
// during the gap between the engine returning instantly (~50 ms) and the
// web-suggestions request settling (~500 ms-2 s).
let queryVersion = 0;
let lastEnginePayload = [];
let lastWebSuggestions = [];
let lastQueryString = null;
let webInFlight = false;

/** Resolved by the backend (Windows: SHGetKnownFolderPath; *nix: $HOME/<name>).
 *  Empty until setQuickFolders is called at boot. */
let quickFolders = [];

export function setOnResults(callback) {
  onResultsCallback = callback;
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

export function isPrefixHintMode() {
  return prefixHintMode;
}

export function isCommandHintMode() {
  return commandHintMode;
}

export function setAiEnabled(enabled) {
  aiEnabled = !!enabled;
  if (!aiEnabled) {
    lastWebSuggestions = [];
    clearTimeout(webSuggestionTimer);
  }
}

export function getTranslateText() {
  return translateMode ? _translateText : '';
}

let _translateText = '';

export function handleQueryInput(query) {
  clearTimeout(debounceTimer);
  clearTimeout(webSuggestionTimer);
  // Bump version + clear stale engine payload. Web-suggestion cache only
  // resets when the query actually changes - on a same-query re-run (window
  // toggle, manual refresh) the previous list is still valid, and we'd
  // rather keep showing it than flash "No results" if the re-fetch is slow
  // or returns empty (DuckDuckGo /ac/ is occasionally flaky).
  queryVersion += 1;
  lastEnginePayload = [];
  if (query !== lastQueryString) {
    lastWebSuggestions = [];
  }
  lastQueryString = query;

  // Discovery menus - `"` lists every query prefix, `:` lists every slash
  // command, both filterable by what follows the leading character. Must
  // run before the t"/c"/rc" branches because the leading chars overlap.
  if (isPrefixSuggestionQuery(query)) {
    prefixHintMode = true;
    commandHintMode = translateMode = clipboardMode = recentMode = false;
    if (onResultsCallback) onResultsCallback(prefixSuggestionResults(query), query);
    return;
  }
  if (isCommandSuggestionQuery(query)) {
    commandHintMode = true;
    prefixHintMode = translateMode = clipboardMode = recentMode = false;
    if (onResultsCallback) onResultsCallback(commandSuggestionResults(query), query);
    return;
  }

  prefixHintMode = false;
  commandHintMode = false;

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
    debounceTimer = setTimeout(() => performSearch(query, queryVersion), DEBOUNCE_MS);
    return;
  }

  recentMode = false;

  const myVersion = queryVersion;
  if (query.trim() === '') {
    performSearch('', myVersion);
    return;
  }

  // Both fetches share the 70 ms debounce and run concurrently - engine
  // results render the moment they land, then web suggestions append below
  // when they arrive. Each leg version-checks before publishing. We mark
  // webInFlight up-front (not inside fetchWebSuggestions) so publish() can
  // see it the instant the engine returns and hold off on painting an
  // empty list. Skip web suggestions entirely for prefixed queries
  // (a"chrome, f"doc, r"regex ...). The engine handles those as scoped
  // filters; Google-autocomplete rows would be noise.
  const wantsWeb = aiEnabled && !isPrefixedQuery(query)
    && query.trim().length >= MIN_WEB_SUGGESTION_QUERY_LENGTH;
  webInFlight = wantsWeb;
  debounceTimer = setTimeout(() => performSearch(query, myVersion), DEBOUNCE_MS);
  if (wantsWeb) {
    webSuggestionTimer = setTimeout(() => fetchWebSuggestions(query, myVersion), DEBOUNCE_MS);
  }
}

// A fetch is "stale" when handleQueryInput has bumped queryVersion since
// the fetch started - used by both legs to bail before mutating shared
// state. Cuts the `if (version !== queryVersion) return;` boilerplate.
function isStale(version) {
  return version !== queryVersion;
}

async function performSearch(query, version) {
  try {
    const payload = await ipcSearch(query, SEARCH_LIMIT);
    if (isStale(version)) return;
    lastEnginePayload = recentMode ? payload.results : prependQuickFolders(payload.results, query);
  } catch (err) {
    console.error('Search failed:', err);
    if (isStale(version)) return;
    lastEnginePayload = [];
  }
  publish(query, version);
}

async function fetchWebSuggestions(query, version) {
  const trimmed = query.trim();
  if (!aiEnabled || trimmed.length < MIN_WEB_SUGGESTION_QUERY_LENGTH) {
    webInFlight = false;
    return;
  }
  try {
    const list = await ipcWebSuggestions(trimmed, WEB_SUGGESTIONS_LIMIT);
    if (isStale(version)) return;
    // Only replace the cached list when the response actually contains
    // suggestions. DDG /ac/ occasionally returns [] on a working query
    // (rate limit, curl glitch); overwriting the cache with that would
    // wipe a perfectly good list and leave the user staring at
    // "No results" on every re-open.
    if (Array.isArray(list) && list.length > 0) {
      lastWebSuggestions = list;
    }
  } catch (err) {
    // Web suggestions are best-effort - silent failure mirrors macOS.
    console.warn('Web suggestions failed:', err);
  } finally {
    if (!isStale(version)) {
      webInFlight = false;
      publish(query, version);
    }
  }
}

function publish(query, version) {
  if (isStale(version) || !onResultsCallback) return;
  const suggestionRows = aiEnabled && lastWebSuggestions.length
    ? webSuggestionResults(lastWebSuggestions)
    : [];
  const combined = [...lastEnginePayload, ...suggestionRows];
  // Hold an empty render while web suggestions are still loading. The
  // engine returns instantly (~50 ms) but DDG /ac/ takes 500 ms-2 s; on a
  // fresh query we'd otherwise paint "No results" for a couple of seconds
  // before the suggestion list pops in. Once both legs have settled (web
  // returned or errored), webInFlight clears and we publish honestly.
  if (combined.length === 0 && webInFlight) return;
  onResultsCallback(combined, query);
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
    const isTrash = folder.title === 'Trash' || folder.title === 'Recycle Bin';
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
