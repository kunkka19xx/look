import {
    search as ipcSearch,
    getClipboardHistory,
    searchProcesses as ipcSearchProcesses,
    webSuggestions as ipcWebSuggestions,
    classifyUrl as ipcClassifyUrl,
    recentUrls as ipcRecentUrls,
    calcInline as ipcCalcInline,
} from './ipc.js';
import {
    isPrefixSuggestionQuery,
    isCommandSuggestionQuery,
    isPrefixedQuery,
    prefixSuggestionResults,
    commandSuggestionResults,
    webSuggestionResults,
    webUrlResult,
    calcResult,
    WEB_URL_OPEN_SUBTITLE,
    WEB_URL_RECENT_SUBTITLE,
} from './catalog.js';
import * as layout from './layout.js';

const DEBOUNCE_MS = 70;
const MIN_QUICK_FOLDER_PREFIX = 2;
const SEARCH_LIMIT = 40;
const WEB_SUGGESTIONS_LIMIT = 6;
const RECENT_URL_LIMIT = 5;
const MIN_WEB_SUGGESTION_QUERY_LENGTH = 2;
const CLIPBOARD_TITLE_MAX_CHARS = 80;
let debounceTimer = null;
let webSuggestionTimer = null;
let onResultsCallback = null;
let clipboardMode = false;
let translateMode = false;
let recentMode = false;
let processMode = false;
// Set after a kill (or on mode entry) so the next process search re-enumerates
// /proc instead of scoring the stale snapshot.
let processRefreshPending = false;
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
// URL rows (issue #232 + url-history spec): the live "Open <url>" match for
// the current query and previously-opened URLs matching it. Reset on every
// input (both are local and fast, so no cross-query caching like the flaky
// web suggestions need) and refilled by fetchUrlRows.
let lastUrlMatch = null;
let lastRecentUrls = [];
// Calculator row for the current query, or null. Local and instant, so unlike
// the other legs it runs undebounced and settles before the engine answers.
let lastCalc = null;

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

export function isProcessMode() {
    return processMode;
}

// Called after a kill so the next process search re-enumerates /proc. Callers
// dispatch an input event to re-run the current query.
export function forceProcessRefresh() {
    processRefreshPending = true;
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
    lastUrlMatch = null;
    lastRecentUrls = [];
    lastCalc = null;
    if (query !== lastQueryString) {
        lastWebSuggestions = [];
    }
    lastQueryString = query;

    // Mutually-exclusive query modes, checked in a fixed order. Each branch
    // below only clears the sibling flags no *earlier* branch already
    // cleared - by the time control reaches the c"/rc"/ps" checks,
    // prefixHintMode/commandHintMode/translateMode are already false, so
    // they're not re-listed there. This is deliberate economy, not
    // partial/inconsistent duplication: reading top-to-bottom, every flag is
    // false by the time a later branch could need to clear it again.
    //
    // Discovery menus - `"` lists every query prefix, `:` lists every slash
    // command, both filterable by what follows the leading character. Must
    // run before the t"/c"/rc" branches because the leading chars overlap.
    if (isPrefixSuggestionQuery(query)) {
        prefixHintMode = true;
        commandHintMode = translateMode = clipboardMode = recentMode = processMode = false;
        if (onResultsCallback) onResultsCallback(prefixSuggestionResults(query), query);
        return;
    }
    if (isCommandSuggestionQuery(query)) {
        commandHintMode = true;
        prefixHintMode = translateMode = clipboardMode = recentMode = processMode = false;
        if (onResultsCallback) onResultsCallback(commandSuggestionResults(query), query);
        return;
    }

    prefixHintMode = false;
    commandHintMode = false;
    // Default off; the ps" branch below re-arms it. Every early-return mode
    // branch (t"/c"/rc") is reached with processMode already cleared here.
    const wasProcessMode = processMode;
    processMode = false;

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

    if (query.startsWith('ps"')) {
        processMode = true;
        // Re-enumerate on mode entry or after a kill; otherwise score the cached
        // snapshot so typing never walks /proc.
        const refresh = processRefreshPending || !wasProcessMode;
        processRefreshPending = false;
        const filter = query.slice(3);
        const version = queryVersion;
        debounceTimer = setTimeout(
            () => performProcessSearch(filter, refresh, version),
            DEBOUNCE_MS,
        );
        return;
    }

    const myVersion = queryVersion;
    if (query.trim() === '') {
        // The rest screen shows no rows, and the engine answers an empty query
        // by scoring the whole index. Clear instead of searching for nothing.
        if (layout.isEmptyQuery(query) && layout.hidesResultsForEmptyQuery()) {
            if (onResultsCallback) onResultsCallback([], query);
            return;
        }
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
    const prefixed = isPrefixedQuery(query);
    const wantsWeb =
        aiEnabled && !prefixed && query.trim().length >= MIN_WEB_SUGGESTION_QUERY_LENGTH;
    webInFlight = wantsWeb;
    // URL rows share the engine debounce: both are local round-trips, and the
    // URL leg must not run per keystroke ahead of it (SQLite open per call).
    // Unlike web suggestions they ignore the AI gate - opening a typed address
    // is a launcher action, not a web answer.
    const wantsUrlRows = !prefixed;
    // No debounce: local arithmetic on a short string, and it settles before
    // the engine, so the row never re-seats the selection.
    if (!prefixed) fetchCalc(query, myVersion);
    debounceTimer = setTimeout(() => {
        performSearch(query, myVersion);
        if (wantsUrlRows) fetchUrlRows(query, myVersion);
    }, DEBOUNCE_MS);
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
        lastEnginePayload = recentMode
            ? payload.results
            : prependQuickFolders(payload.results, query);
    } catch (err) {
        console.error('Search failed:', err);
        if (isStale(version)) return;
        lastEnginePayload = [];
    }
    publish(query, version);
}

async function fetchWebSuggestions(query, version) {
    // Only aiEnabled is worth re-checking here: it's a mutable setting that
    // can flip during the debounce window between scheduling and firing,
    // unlike the length gate, which already tested this exact (immutable)
    // query string as part of `wantsWeb` before the timer was even set.
    if (!aiEnabled) {
        webInFlight = false;
        return;
    }
    const trimmed = query.trim();
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

// core/calc decides what counts, so `20-05-2026` stays a folder name.
async function fetchCalc(query, version) {
    try {
        const result = await ipcCalcInline(query);
        if (isStale(version)) return;
        lastCalc = result || null;
    } catch (err) {
        console.warn('Calc failed:', err);
        if (isStale(version)) return;
        lastCalc = null;
    }
    if (lastCalc) publish(query, version);
}

// Classification + history lookup for the current query. Both run in one leg
// so a single publish paints them together; each response is version-checked
// like the other legs. Best-effort: on failure the query simply has no URL
// rows.
async function fetchUrlRows(query, version) {
    try {
        const [match, recents] = await Promise.all([
            ipcClassifyUrl(query),
            ipcRecentUrls(query.trim(), RECENT_URL_LIMIT),
        ]);
        if (isStale(version)) return;
        lastUrlMatch = match || null;
        lastRecentUrls = Array.isArray(recents) ? recents : [];
    } catch (err) {
        console.warn('URL rows failed:', err);
        if (isStale(version)) return;
        lastUrlMatch = null;
        lastRecentUrls = [];
    }
    publish(query, version);
}

// Stable score-descending merge of engine results and recent-URL rows, so a
// frequently/recently opened URL rises exactly as far as its frecency earns
// against local matches. Both inputs are already score-sorted; on ties the
// local result stays ahead, so a URL never displaces an equally-ranked
// app/file. Mirrors macOS LauncherView+URLResults.mergeByScore.
function mergeByScore(local, recents) {
    if (recents.length === 0) return local;
    const merged = [];
    let i = 0;
    let j = 0;
    while (i < local.length && j < recents.length) {
        if (recents[j].score > local[i].score) {
            merged.push(recents[j]);
            j += 1;
        } else {
            merged.push(local[i]);
            i += 1;
        }
    }
    merged.push(...local.slice(i), ...recents.slice(j));
    return merged;
}

function publish(query, version) {
    if (isStale(version) || !onResultsCallback) return;
    const suggestionRows =
        aiEnabled && lastWebSuggestions.length ? webSuggestionResults(lastWebSuggestions) : [];
    // Recent URLs interleave with engine results by frecency, deduped against
    // the live row so the same address never appears twice. The live row's
    // placement follows its tier (issue #232): a structural match (scheme,
    // port, path, localhost/IP) can't be a file or search, so it ranks top; a
    // bare host.tld must never steal the default slot from a real local
    // result, so it sits after them. Web-search rows stay last.
    const liveUrl = lastUrlMatch ? lastUrlMatch.url : null;
    const recentRows = lastRecentUrls
        .filter((e) => e.url !== liveUrl)
        .map((e) => webUrlResult(e.url, WEB_URL_RECENT_SUBTITLE, e.score));
    const ranked = mergeByScore(lastEnginePayload, recentRows);
    let combined;
    if (!lastUrlMatch) {
        combined = [...ranked, ...suggestionRows];
    } else {
        const liveRow = webUrlResult(liveUrl, WEB_URL_OPEN_SUBTITLE, 0);
        combined =
            lastUrlMatch.tier === 'structural'
                ? [liveRow, ...ranked, ...suggestionRows]
                : [...ranked, liveRow, ...suggestionRows];
    }
    // Above everything and takes the selection: an expression is a question,
    // and the answer outranks a file that fuzzy-matched some of its digits.
    if (lastCalc) {
        combined = [calcResult(query.trim(), lastCalc), ...combined];
    }
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
    const months = [
        'Jan',
        'Feb',
        'Mar',
        'Apr',
        'May',
        'Jun',
        'Jul',
        'Aug',
        'Sep',
        'Oct',
        'Nov',
        'Dec',
    ];
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
            const title =
                e.text.replace(/\n/g, '  ').slice(0, CLIPBOARD_TITLE_MAX_CHARS) || '(empty)';
            const shortDate = formatShortDate(e.timestamp);
            const subtitle = `Clipboard  \u2022  ${e.char_count} chars  \u2022  ${e.line_count} lines  \u2022  ${shortDate}`;
            return {
                id: `clip:${e.timestamp}:${i}`,
                kind: 'clipboard',
                title,
                subtitle,
                path: 'clipboard://history',
                score: 0,
                clipText: e.text,
                clipPayload: e.payload || null,
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

async function performProcessSearch(filter, refresh, version) {
    try {
        const procs = await ipcSearchProcesses(filter.trim(), !!refresh);
        // A newer query or a mode switch may have landed while /proc was being
        // walked; discard this response so it can't overwrite fresher results.
        if (isStale(version)) return;
        const results = procs.map((p) => {
            const ports = p.ports || [];
            const portHint = ports.length ? ` \u2022 ${ports.map((x) => `:${x}`).join(' ')}` : '';
            return {
                id: `proc:${p.pid}`,
                kind: 'process',
                title: p.name,
                subtitle: `PID ${p.pid}${portHint}`,
                path: `process://${p.pid}`,
                score: 0,
                procPid: p.pid,
                procName: p.name,
                // .desktop path when the process belongs to an app; drives the icon.
                iconPath: p.icon_source || null,
                procPorts: ports,
            };
        });
        if (onResultsCallback) onResultsCallback(results, `ps"${filter}`);
    } catch (err) {
        if (isStale(version)) return;
        console.error('Process search failed:', err);
        if (onResultsCallback) onResultsCallback([], `ps"${filter}`);
    }
}

export { formatMediumDate };

function prependQuickFolders(results, query) {
    if (quickFolders.length === 0) return results;
    const q = query.toLowerCase().trim();
    if (q.length < MIN_QUICK_FOLDER_PREFIX) return results;

    const merged = [...results];
    let insertAt = 0;

    for (const folder of quickFolders) {
        if (!folder.title.toLowerCase().startsWith(q)) continue;
        if (merged.some((r) => r.path === folder.path)) continue;

        const isTrash = folder.title === 'Trash' || folder.title === 'Recycle Bin';
        const quickFolderResult = {
            id: `quickfolder:${folder.title.toLowerCase()}`,
            kind: 'folder',
            title: folder.title,
            subtitle: isTrash ? 'Pinned · Ctrl+D to empty' : 'Pinned home folder',
            path: folder.path,
            score: 999999,
        };

        // A same-titled app already won the backend's type-priority ranking -
        // don't let the pin bump it out of first place. Surface the folder
        // right below it instead of dropping it, so it's still reachable.
        const rivalAppIndex = merged.findIndex(
            (r) => r.kind === 'app' && r.title.toLowerCase() === folder.title.toLowerCase()
        );
        if (rivalAppIndex === -1) {
            merged.splice(insertAt, 0, quickFolderResult);
            insertAt += 1;
        } else {
            merged.splice(rivalAppIndex + 1, 0, quickFolderResult);
        }
    }

    return merged;
}
