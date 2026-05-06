import * as results from './components/results.js';
import { openPath, recordUsage, revealPath, hideWindow, copyFilesToClipboard } from './ipc.js';

let queryInput = null;
let shiftHeld = false;

export function init(inputEl) {
  queryInput = inputEl;

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

function handleKeyDown(e) {
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
      if (e.ctrlKey) {
        searchWeb();
      } else {
        openSelected();
      }
      break;

    case 'Escape':
      e.preventDefault();
      hideWindow();
      break;

    case 'f':
      if (e.ctrlKey) {
        e.preventDefault();
        revealSelected();
      }
      break;

    case 'c':
      if (e.ctrlKey && !window.getSelection()?.toString()) {
        e.preventDefault();
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
        results.togglePick(results.getSelected());
      }
      break;
  }
}

async function openSelected() {
  const item = results.getSelected();
  if (!item) return;

  try {
    await openPath(item.path, item.kind, item.id);

    // Determine action type from kind
    const actionMap = {
      app: 'open_app',
      file: 'open_file',
      folder: 'open_folder',
    };
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
      await navigator.clipboard.writeText(item.path);
    }
  } catch (err) {
    console.error('Failed to copy:', err);
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
