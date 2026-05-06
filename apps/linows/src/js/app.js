import * as results from './components/results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import * as preview from './components/preview.js';
import * as picked from './components/picked.js';
import { onWindowShown, getHomeDir, copyFilesToClipboard } from './ipc.js';

document.addEventListener('DOMContentLoaded', () => {
  const queryInput = document.getElementById('query');
  const resultsList = document.getElementById('results-list');
  const previewPanel = document.getElementById('preview-panel');

  // Initialize modules
  results.init(resultsList);
  keyboard.init(queryInput);
  preview.init(previewPanel);
  picked.init(previewPanel, {
    onRemoveItem: (key) => results.removePick(key),
    onClearAll: () => results.clearPicks(),
  });

  // Update right panel when selection changes
  results.setOnSelectionChange((item) => {
    if (!results.hasPickedItems()) {
      preview.update(item);
    }
  });

  // Update right panel when picks change + auto-copy
  results.setOnPickChange((pickedItems) => {
    if (pickedItems.length > 0) {
      preview.clear();
      picked.update(pickedItems);
      // Auto-copy picked files to clipboard
      const paths = pickedItems
        .filter((i) => i.kind === 'file' || i.kind === 'folder')
        .map((i) => i.path);
      if (paths.length > 0) {
        copyFilesToClipboard(paths).catch(() => {});
      }
    } else {
      picked.update([]);
      preview.update(results.getSelected());
    }
  });

  // Wire search → results
  search.setOnResults((items, query) => {
    results.render(items);
  });

  // Search on input
  queryInput.addEventListener('input', (e) => {
    search.handleQueryInput(e.target.value);
  });

  // Click on result row → open
  resultsList.addEventListener('result-activate', () => {
    const item = results.getSelected();
    if (item) {
      import('./ipc.js').then(({ openPath, recordUsage }) => {
        openPath(item.path, item.kind, item.id);
        const actionMap = { app: 'open_app', file: 'open_file', folder: 'open_folder' };
        recordUsage(item.id, actionMap[item.kind] || 'open_file');
      });
    }
  });

  // When window shown via global hotkey, focus input and select all
  onWindowShown(() => {
    queryInput.focus();
    queryInput.select();
  });

  // Load home dir for quick folders, then initial search
  getHomeDir().then((home) => {
    if (home) search.setHomeDir(home);
    search.handleQueryInput('');
  });
});
