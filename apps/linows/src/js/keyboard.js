import * as results from './results.js';
import { openPath, recordUsage, revealPath, hideWindow } from './ipc.js';

let queryInput = null;

export function init(inputEl) {
  queryInput = inputEl;

  document.addEventListener('keydown', handleKeyDown);
}

function handleKeyDown(e) {
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
      openSelected();
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
  }
}

async function openSelected() {
  const item = results.getSelected();
  if (!item) return;

  try {
    await openPath(item.path);

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

async function revealSelected() {
  const item = results.getSelected();
  if (!item) return;

  try {
    await revealPath(item.path);
  } catch (err) {
    console.error('Failed to reveal:', err);
  }
}
