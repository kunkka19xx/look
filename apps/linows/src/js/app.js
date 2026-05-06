import * as results from './results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import * as preview from './preview.js';
import { onWindowShown, getHomeDir } from './ipc.js';

document.addEventListener('DOMContentLoaded', () => {
  const queryInput = document.getElementById('query');
  const resultsList = document.getElementById('results-list');
  const previewPanel = document.getElementById('preview-panel');

  // Initialize modules
  results.init(resultsList);
  keyboard.init(queryInput);
  preview.init(previewPanel);

  // Update preview when selection changes
  results.setOnSelectionChange((item) => {
    preview.update(item);
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
