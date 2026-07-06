import * as results from './components/results.js';
import * as search from './search.js';
import * as translatePanel from './components/translate.js';
import { openPath, recordUsage, revealPath, hideWindow, copyFilesToClipboard, copyToClipboard, deleteClipboardEntry, trashPaths, countTrashItems, emptyTrash, requestIndexRefresh } from './ipc.js';
import * as banner from './components/banner.js';
import * as confirm from './components/confirm.js';
import * as runningApps from './components/running-apps.js';
import { trash as trashIcon } from './icons.js';
import { prefixFromResultId, commandIdFromResultId, webSuggestionFromResultId } from './catalog.js';
import * as platform from './platform.js';

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

export function init(inputEl) {
  queryInput = inputEl;
  helpScreen = document.getElementById('help-screen');

  // Disable tab-focusability on everything except the search input
  // so WebKitGTK doesn't intercept Shift+Tab for focus cycling
  document.querySelectorAll('*').forEach((el) => {
    if (el !== inputEl) el.tabIndex = -1;
  });

  // Track Shift key state independently (webview may strip shiftKey from Tab events)
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Shift') shiftHeld = true;
  }, true);
  document.addEventListener('keyup', (e) => {
    if (e.key === 'Shift') shiftHeld = false;
  }, true);

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
    import('./ipc.js').then(m => m.quitApp());
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
      } else if (e.ctrlKey) {
        searchWeb();
      } else if ((e.shiftKey || shiftHeld) && results.hasPickedItems()) {
        openAllPicked();
      } else {
        openSelected();
      }
      break;

    case 'Delete':
    case 'Backspace':
      if (search.isClipboardMode() && e.key === 'Delete') {
        e.preventDefault();
        removeClipboardEntry();
      }
      break;

    case 'Escape':
      e.preventDefault();
      if (search.isClipboardMode() || search.isTranslateMode()
          || search.isPrefixHintMode() || search.isCommandHintMode()) {
        queryInput.value = '';
        translatePanel.hide();
        queryInput.dispatchEvent(new Event('input'));
        queryInput.focus();
      } else {
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
        copySelectedPath();
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
      if (e.ctrlKey && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        if (isDiscoveryMode()) break;
        handleTrashShortcut();
      }
      break;
  }
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
    try { await requestIndexRefresh(); } catch (_) {}
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
    try { await requestIndexRefresh(); } catch (_) {}
  } catch (err) {
    banner.show(`Empty ${label} failed: ${err}`, 'error', 2.0);
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

async function openSelected() {
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
  // Google autocomplete row → open the search in the browser.
  const suggestionText = webSuggestionFromResultId(item.id);
  if (suggestionText != null) {
    const url = `https://www.google.com/search?q=${encodeURIComponent(suggestionText)}`;
    openPath(url, 'browser', '');
    return;
  }

  try {
    await openPath(item.path, item.kind, item.id);
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
    await copyToClipboard(item.clipText);
    banner.show('Copied to clipboard', 'success', 1.0);
  } catch (err) {
    banner.show('Copy failed', 'error', 1.2);
  }
}

function toggleHelp() {
  if (!helpScreen) return;
  helpScreen.hidden = !helpScreen.hidden;
}

async function removeClipboardEntry() {
  const item = results.getSelected();
  if (!item || item.kind !== 'clipboard') return;
  try {
    await deleteClipboardEntry(item.clipIndex);
    // Re-trigger search to refresh the list
    search.handleQueryInput(queryInput.value);
  } catch (err) {
    console.error('Delete clipboard entry failed:', err);
  }
}
