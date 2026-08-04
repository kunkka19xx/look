import * as results from './components/results.js';
import * as search from './search.js';
import * as translatePanel from './components/translate.js';
import {
    openPath,
    openElevated,
    recordUsage,
    recordUrlHit,
    revealPath,
    hideWindow,
    copyFilesToClipboard,
    copyToClipboard,
    copyToClipboardLabeled,
    deleteClipboardEntry,
    killProcess,
    trashPaths,
    countTrashItems,
    emptyTrash,
    requestIndexRefresh,
    getConfig,
    setConfig,
    reloadConfig,
} from './ipc.js';
import * as preview from './components/preview.js';
import * as banner from './components/banner.js';
import * as confirm from './components/confirm.js';
import * as qactions from './components/qactions.js';
import * as superactions from './components/superactions.js';
import * as runningApps from './components/running-apps.js';
import { canRunElevated } from './platform.js';
import { trash as trashIcon } from './icons.js';
import {
    prefixFromResultId,
    commandIdFromResultId,
    webSuggestionFromResultId,
    webUrlFromResultId,
    calcRawFromResultId,
    isSyntheticResultId,
} from './catalog.js';
import * as platform from './platform.js';
import * as layout from './layout.js';

// The quick-folder pin for the OS trash: `Trash` on Linux/macOS,
// `Recycle Bin` on Windows (id is `quickfolder:<lowercased title>`).
const TRASH_PIN_IDS = ['quickfolder:trash', 'quickfolder:recycle bin'];

let queryInput = null;
let shiftHeld = false;
let commandMode = null;
let enterCommandModeFn = null;
let settingsModule = null;
let settingsContentArea = null;
let settingsSearchBar = null;
let helpScreen = null;
// Re-asserts the empty-state control strip after a screen open/close (set by
// app.js). The strip must step aside for settings/help and return afterwards.
let syncHomeFn = null;

export function init(inputEl) {
    queryInput = inputEl;
    helpScreen = document.getElementById('help-screen');

    // Disable tab-focusability on everything except the search input
    // so WebKitGTK doesn't intercept Shift+Tab for focus cycling
    document.querySelectorAll('*').forEach((el) => {
        if (el !== inputEl) el.tabIndex = -1;
    });

    // Track Shift key state independently (webview may strip shiftKey from Tab events)
    document.addEventListener(
        'keydown',
        (e) => {
            if (e.key === 'Shift') shiftHeld = true;
        },
        true,
    );
    document.addEventListener(
        'keyup',
        (e) => {
            if (e.key === 'Shift') shiftHeld = false;
        },
        true,
    );

    document.addEventListener('keydown', handleKeyDown, true);
}

export function setCommandMode(cmdModule) {
    commandMode = cmdModule;
}

export function setEnterCommandMode(fn) {
    enterCommandModeFn = fn;
}

export function setSettingsMode(mod, contentArea, searchBar) {
    settingsModule = mod;
    settingsContentArea = contentArea;
    settingsSearchBar = searchBar;
}

export function setSyncHome(fn) {
    syncHomeFn = fn;
}

function handleKeyDown(e) {
    if (confirm.isActive()) {
        const k = e.key;
        if (k === 'y' || k === 'Y' || k === 'Enter') {
            e.preventDefault();
            confirm.confirm();
            return;
        }
        if (k === 'n' || k === 'N' || k === 'Escape') {
            e.preventDefault();
            confirm.cancel();
            return;
        }
        e.preventDefault();
        return;
    }

    // Alt+Shift+Q quits the app
    if (e.altKey && (e.shiftKey || shiftHeld) && (e.key === 'Q' || e.key === 'q')) {
        e.preventDefault();
        import('./ipc.js').then((m) => m.quitApp());
        return;
    }

    // Ctrl+Shift+, toggles settings
    if (e.ctrlKey && (e.shiftKey || shiftHeld) && (e.key === ',' || e.key === '<')) {
        e.preventDefault();
        if (settingsModule?.isActive()) {
            settingsModule.exit(settingsContentArea, settingsSearchBar);
        } else {
            // Exit command mode first if active
            if (commandMode?.isActive()) commandMode.exit();
            settingsModule.enter(settingsContentArea, settingsSearchBar);
        }
        syncHomeFn?.();
        return;
    }

    // Ctrl+Shift+; reloads config from file (like Cmd+Shift+; on macOS)
    if (e.ctrlKey && (e.shiftKey || shiftHeld) && (e.key === ';' || e.key === ':')) {
        e.preventDefault();
        if (settingsModule) settingsModule.reloadFromFile();
        return;
    }

    // Ctrl+H toggles help screen (only outside command mode)
    if (e.ctrlKey && !e.shiftKey && e.key === 'h') {
        if (!commandMode?.isActive()) {
            e.preventDefault();
            toggleHelp();
            return;
        }
    }

    // Ctrl+Shift+H: hide the selected app from Look
    if (e.ctrlKey && (e.shiftKey || shiftHeld) && (e.key === 'H' || e.key === 'h')) {
        e.preventDefault();
        if (settingsModule?.isActive()) return;
        if (helpScreen && !helpScreen.hidden) return;
        if (commandMode?.isActive()) return;
        void handleHideSelectApp();
        return;
    }

    // Ctrl+= / Ctrl+- / Ctrl+0 - UI zoom in/out/reset. Mirrors macOS
    // Cmd+= / Cmd+- / Cmd+0 (apps/macos/.../look_appApp.swift:177). Global:
    // works in search, command, settings, and help screens.
    if (e.ctrlKey && !e.shiftKey && !e.altKey && settingsModule) {
        if (e.key === '=') {
            e.preventDefault();
            settingsModule.zoomIn();
            return;
        }
        if (e.key === '-') {
            e.preventDefault();
            settingsModule.zoomOut();
            return;
        }
        if (e.key === '0') {
            e.preventDefault();
            settingsModule.resetZoom();
            return;
        }
    }

    // Delegate to settings if active
    if (settingsModule?.isActive()) {
        if (settingsModule.handleKey(e)) return;
        return;
    }

    // Help screen: Esc closes it
    if (helpScreen && !helpScreen.hidden) {
        if (e.key === 'Escape') {
            e.preventDefault();
            helpScreen.hidden = true;
            layout.setModal('help', false);
            syncHomeFn?.();
            return;
        }
        return; // swallow all other keys while help is open
    }

    // Ctrl+/ toggles command mode
    if (e.ctrlKey && (e.key === '/' || e.key === '?')) {
        e.preventDefault();
        if (commandMode?.isActive()) {
            commandMode.exit();
        } else if (enterCommandModeFn) {
            enterCommandModeFn();
        }
        return;
    }

    // Delegate to command mode if active
    if (commandMode?.isActive()) {
        if (commandMode.handleKey(e)) return;
        // Let typing through to input
        return;
    }

    // Alt+1-9 on home screen → activate running app
    if (e.altKey && !e.ctrlKey && !e.shiftKey && e.key >= '1' && e.key <= '9') {
        const num = parseInt(e.key);
        if (runningApps.activateByKey(num)) {
            e.preventDefault();
            return;
        }
    }

    // Alt+<char> on the empty-state home screen → fire the super action whose
    // highlighted mnemonic matches. Mirrors Cmd+<char> on the macOS launchpad.
    // Gated on the strip being visible so it never shadows typing or other Alt
    // chords when results are showing.
    if (
        e.altKey &&
        !e.ctrlKey &&
        !e.metaKey &&
        !e.shiftKey &&
        !shiftHeld &&
        superactions.isVisible()
    ) {
        const ch = mnemonicChar(e);
        if (ch && superactions.handleMnemonic(ch)) {
            e.preventDefault();
            return;
        }
    }

    // WebKitGTK reports Shift+Tab as key="Unidentified", code="Tab"
    if (e.key === 'Tab' || (e.code === 'Tab' && e.key === 'Unidentified')) {
        e.preventDefault();
        e.stopPropagation();
        if (e.shiftKey || shiftHeld) {
            results.selectPrev();
        } else {
            results.selectNext();
        }
        queryInput.focus();
        return;
    }

    switch (e.key) {
        case 'ArrowDown':
            e.preventDefault();
            results.selectNext();
            break;

        case 'ArrowUp':
            e.preventDefault();
            results.selectPrev();
            break;

        case 'Enter':
            e.preventDefault();
            if (search.isTranslateMode()) {
                const text = search.getTranslateText();
                if (text) translatePanel.perform(text);
            } else if (search.isClipboardMode()) {
                copyClipboardEntry();
            } else if (search.isProcessMode()) {
                // ps": Enter measures CPU on demand (kill is Ctrl+D). Keeps
                // selection instant by never sampling until asked.
                preview.measureCpu();
            } else if (
                e.ctrlKey &&
                (e.shiftKey || shiftHeld) &&
                canRunElevated(results.getSelected())
            ) {
                // Ctrl+Shift+Enter: launch the selected app elevated (UAC).
                openSelected(true);
            } else if (e.ctrlKey) {
                searchWeb();
            } else if ((e.shiftKey || shiftHeld) && results.hasPickedItems()) {
                openAllPicked();
            } else {
                openSelected();
            }
            break;

        case 'Escape':
            e.preventDefault();
            if (
                search.isClipboardMode() ||
                search.isTranslateMode() ||
                search.isProcessMode() ||
                search.isPrefixHintMode() ||
                search.isCommandHintMode()
            ) {
                queryInput.value = '';
                translatePanel.hide();
                queryInput.dispatchEvent(new Event('input'));
                queryInput.focus();
            } else {
                // Arm here: Rust's window-hidden can lose the race with hide().
                superactions.armEntrance();
                hideWindow();
            }
            break;

        case 'f':
            if (e.ctrlKey) {
                e.preventDefault();
                if (isDiscoveryMode()) break;
                revealSelected();
            }
            break;

        case 'c':
            if (e.ctrlKey && !window.getSelection()?.toString()) {
                e.preventDefault();
                if (isDiscoveryMode()) break;
                if (search.isProcessMode()) {
                    copySelectedPid();
                } else {
                    copySelectedPath();
                }
            }
            break;

        case 'p':
        case 'P':
            if (e.ctrlKey && (e.shiftKey || shiftHeld)) {
                e.preventDefault();
                results.clearPicks();
            } else if (e.ctrlKey) {
                e.preventDefault();
                if (isDiscoveryMode()) break;
                // Only files/folders are pickable - apps/settings/clipboard rows have
                // no real path to copy and would leave the picked panel rendering
                // nonsense. Mirrors macOS togglePickForSelectedResult.
                const sel = results.getSelected();
                if (!sel) break;
                if (sel.kind !== 'file' && sel.kind !== 'folder') {
                    banner.show('Only files or folders can be picked', 'info', 1.2);
                    break;
                }
                results.togglePick(sel);
            }
            break;

        case 'd':
        case 'D':
            if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
                e.preventDefault();
                if (isDiscoveryMode()) break;
                if (search.isProcessMode()) {
                    killSelectedProcess();
                } else if (search.isClipboardMode()) {
                    removeClipboardEntry();
                } else {
                    handleTrashShortcut();
                }
            }
            break;

        case 'o':
            // Ctrl+O flips the selected result's toggle Quick Action
            // (Bluetooth, ...). Mirrors Cmd+O on macOS; no-op when the
            // selection has none.
            if (e.ctrlKey && !e.shiftKey && !e.altKey) {
                e.preventDefault();
                qactions.togglePrimary();
            }
            break;
    }
}

// The letter behind an Alt chord. Prefer e.key so it respects the layout (like
// macOS charactersIgnoringModifiers); fall back to the physical KeyX code when
// Alt composed the key into a dead or non-letter value on some layouts.
function mnemonicChar(e) {
    if (/^[a-z]$/i.test(e.key)) return e.key.toLowerCase();
    const m = /^Key([A-Z])$/.exec(e.code);
    return m ? m[1].toLowerCase() : null;
}

// Side actions (reveal, copy path, pick, trash) don't make sense on synthetic
// discovery rows - their `path` is empty. Mirrors macOS guards on
// revealSelectedInFinder / togglePickForSelectedResult.
function isDiscoveryMode() {
    return search.isPrefixHintMode() || search.isCommandHintMode();
}

function trashTargetsFromSelection() {
    const picked = results.getPickedItems();
    const candidates = picked.length > 0 ? picked : [results.getSelected()].filter(Boolean);
    return candidates.filter((item) => item.kind === 'file' || item.kind === 'folder');
}

// Mirror the backend CSV contract for config lists: `\,` is a literal comma and
// `\\` is a literal backslash. A naive split(',') would corrupt existing
// escaped entries in `app_exclude_names`.
function parseConfigList(value) {
    const entries = [];
    let current = '';

    for (let i = 0; i < value.length; i += 1) {
        const ch = value[i];
        const next = value[i + 1];

        if (ch === '\\' && (next === ',' || next === '\\')) {
            current += next;
            i += 1;
            continue;
        }

        if (ch === ',') {
            const entry = current.trim();
            if (entry) entries.push(entry);
            current = '';
            continue;
        }

        current += ch;
    }

    const entry = current.trim();
    if (entry) entries.push(entry);
    return entries;
}

// Inverse of `parseConfigList`: escape commas/backslashes before joining so an
// app name containing `,` or `\` round-trips through the config file intact.
function renderConfigList(entries) {
    return entries.map((entry) => entry.replaceAll('\\', '\\\\').replaceAll(',', '\\,')).join(',');
}

async function handleTrashShortcut() {
    const selected = results.getSelected();
    if (selected && typeof selected.id === 'string' && TRASH_PIN_IDS.includes(selected.id)) {
        await handleEmptyTrash();
        return;
    }

    const targets = trashTargetsFromSelection();
    if (targets.length === 0) {
        banner.show('Select a file or folder to delete', 'info', 1.2);
        return;
    }

    try {
        const outcome = await trashPaths(targets.map((t) => t.path));
        results.clearPicks();
        const label = platform.trashLabel();
        if (outcome.failed.length === 0) {
            banner.show(`Moved ${outcome.trashed} to ${label}`, 'success', 1.4);
        } else if (outcome.trashed === 0) {
            const first = outcome.failed[0];
            const name = first.path.split(/[\\/]/).pop() || first.path;
            banner.show(`Failed to trash ${name}: ${first.reason}`, 'error', 2.0);
        } else {
            banner.show(`Moved ${outcome.trashed}, ${outcome.failed.length} failed`, 'error', 2.0);
        }
        try {
            await requestIndexRefresh();
        } catch (_) {}
    } catch (err) {
        banner.show(`Trash failed: ${err}`, 'error', 2.0);
    }
}

async function handleEmptyTrash() {
    const label = platform.trashLabel();
    let count;
    try {
        count = await countTrashItems();
    } catch (err) {
        banner.show(`Empty ${label} unavailable: ${err}`, 'error', 2.2);
        return;
    }
    if (count === 0) {
        banner.show(`${label} is already empty`, 'info', 1.2);
        return;
    }
    const itemWord = count === 1 ? 'item' : 'items';
    const ok = await confirm.ask({
        title: `Empty ${label}?`,
        detail: `${count} ${itemWord} - deleted permanently`,
        icon: trashIcon,
    });
    if (!ok) return;
    try {
        const purged = await emptyTrash();
        banner.show(`Emptied ${label} (${purged})`, 'success', 1.4);
        try {
            await requestIndexRefresh();
        } catch (_) {}
    } catch (err) {
        banner.show(`Empty ${label} failed: ${err}`, 'error', 2.0);
    }
}

async function handleHideSelectApp() {
    const item = results.getSelected();
    // Only real launcher apps carry a path; synthetic rows must not be excluded.
    if (!item || item.kind !== 'app' || !item.path || isSyntheticResultId(item.id)) {
        banner.show('Select an app first', 'warning', 1.2);
        return;
    }

    const ok = await confirm.ask({
        title: 'Hide this app from Look?',
        detail: `${item.title} will be added to app_exclude_names`,
    });
    if (!ok) return;

    try {
        const cfg = await getConfig();
        const entry = cfg.entries.find((e) => e.key === 'app_exclude_names');
        const current = entry?.value ?? '';
        const names = parseConfigList(current);
        const trimmedTitle = item.title.trim();
        const normalizedTitle = trimmedTitle.toLowerCase();

        if (names.some((name) => name.trim().toLowerCase() === normalizedTitle)) {
            banner.show(`${item.title} is already hidden`, 'info', 1.2);
            return;
        }

        names.push(trimmedTitle);

        await setConfig([{ key: 'app_exclude_names', value: renderConfigList(names) }]);
        await reloadConfig();

        banner.show(`Hidden ${item.title}`, 'success', 1.2);
    } catch (err) {
        banner.show(`Hide app failed: ${err}`, 'error', 1.6);
    }
}

export async function openAllPicked() {
    const items = results.getPickedItems();
    if (items.length === 0) return;
    const actionMap = { app: 'open_app', file: 'open_file', folder: 'open_folder' };
    for (const item of items) {
        try {
            await openPath(item.path, item.kind, item.id);
            await recordUsage(item.id, actionMap[item.kind] || 'open_file');
        } catch (err) {
            console.error('Failed to open picked item:', item.path, err);
        }
    }
    results.clearPicks();
}

async function openSelected(elevated = false) {
    const item = results.getSelected();
    if (!item) return;

    // Discovery rows: `prefixhint:` fills the query with that prefix (cursor
    // ready for the term); `cmdhint:` enters the command's panel with empty
    // input. Mirrors macOS openSelectedApp.
    const hintedPrefix = prefixFromResultId(item.id);
    if (hintedPrefix != null) {
        queryInput.value = hintedPrefix;
        queryInput.focus();
        queryInput.setSelectionRange(hintedPrefix.length, hintedPrefix.length);
        queryInput.dispatchEvent(new Event('input'));
        return;
    }
    const hintedCmd = commandIdFromResultId(item.id);
    if (hintedCmd != null && commandMode && enterCommandModeFn) {
        commandMode.enterById(hintedCmd);
        enterCommandModeFn();
        queryInput.value = '';
        return;
    }
    // Calculator row → ungrouped answer to the clipboard, launcher out of the
    // way. History keeps the working (`2+2 = 4`); the paste is the number.
    const calcRaw = calcRawFromResultId(item.id);
    if (calcRaw != null) {
        await copyToClipboardLabeled(calcRaw, `${item.calcExpr} = ${item.title}`);
        hideWindow();
        return;
    }
    // Google autocomplete row → open the search in the browser.
    const suggestionText = webSuggestionFromResultId(item.id);
    if (suggestionText != null) {
        const url = `https://www.google.com/search?q=${encodeURIComponent(suggestionText)}`;
        openPath(url, 'browser', '');
        return;
    }
    // URL row (live or from history) → open the resolved address in the
    // default browser and remember it for faster re-open later. Recording is
    // fire-and-forget; a store failure never blocks the open.
    const urlTarget = webUrlFromResultId(item.id);
    if (urlTarget != null) {
        openPath(urlTarget, 'browser', '');
        recordUrlHit(urlTarget);
        return;
    }

    try {
        if (elevated) {
            await openElevated(item.path);
        } else {
            await openPath(item.path, item.kind, item.id);
        }
        const actionMap = { app: 'open_app', file: 'open_file', folder: 'open_folder' };
        const action = actionMap[item.kind] || 'open_file';
        await recordUsage(item.id, action);
    } catch (err) {
        console.error('Failed to open:', err);
    }
}

function searchWeb() {
    const query = queryInput.value.trim();
    if (!query) return;
    const url = `https://www.google.com/search?q=${encodeURIComponent(query)}`;
    openPath(url, 'browser');
}

async function copySelectedPath() {
    const item = results.getSelected();
    if (!item) return;

    try {
        if (item.kind === 'file' || item.kind === 'folder') {
            await copyFilesToClipboard([item.path]);
        } else {
            // Use backend copy so it marks as self-write
            await copyToClipboard(item.path);
        }
        banner.show('Copied to clipboard', 'success', 1.0);
    } catch (err) {
        banner.show('Copy failed', 'error', 1.2);
    }
}

async function revealSelected() {
    const item = results.getSelected();
    if (!item) return;

    try {
        await revealPath(item.path);
    } catch (err) {
        console.error('Failed to reveal:', err);
    }
}

async function copyClipboardEntry() {
    const item = results.getSelected();
    if (!item || item.kind !== 'clipboard') return;
    try {
        // Labelled entries (calculator results) paste their value, not their label.
        await copyToClipboard(item.clipPayload || item.clipText);
        banner.show('Copied to clipboard', 'success', 1.0);
    } catch (err) {
        banner.show('Copy failed', 'error', 1.2);
    }
}

function toggleHelp() {
    if (!helpScreen) return;
    helpScreen.hidden = !helpScreen.hidden;
    layout.setModal('help', !helpScreen.hidden);
    syncHomeFn?.();
}

async function removeClipboardEntry() {
    const item = results.getSelected();
    if (!item || item.kind !== 'clipboard') return;
    try {
        await deleteClipboardEntry(item.clipTimestamp, item.clipText);
        // Re-trigger search to refresh the list
        search.handleQueryInput(queryInput.value);
    } catch (err) {
        console.error('Delete clipboard entry failed:', err);
    }
}

async function copySelectedPid() {
    const item = results.getSelected();
    if (!item || item.kind !== 'process') return;
    try {
        await copyToClipboard(String(item.procPid));
        banner.show(`Copied PID ${item.procPid}`, 'success', 1.0);
    } catch (err) {
        banner.show('Copy failed', 'error', 1.2);
    }
}

async function killSelectedProcess() {
    const item = results.getSelected();
    if (!item || item.kind !== 'process') return;
    try {
        await killProcess(item.procPid);
        banner.show(`Killed ${item.procName} (${item.procPid})`, 'success', 1.2);
        // Force a fresh /proc walk so the killed row drops off next render.
        search.forceProcessRefresh();
        search.handleQueryInput(queryInput.value);
    } catch (err) {
        banner.show(`Kill failed: ${err}`, 'error', 1.6);
    }
}
