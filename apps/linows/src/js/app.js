import * as results from './results.js';
import * as search from './search.js';
import * as keyboard from './keyboard.js';
import { onWindowShown } from './ipc.js';

document.addEventListener('DOMContentLoaded', () => {
  const queryInput = document.getElementById('query');
  const resultsList = document.getElementById('results-list');

  // Initialize modules
  results.init(resultsList);
  keyboard.init(queryInput);

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
    // Keyboard module handles open via Enter; click triggers same event
    const item = results.getSelected();
    if (item) {
      import('./ipc.js').then(({ openPath, recordUsage }) => {
        openPath(item.path);
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

  // Initial empty-query search (browse mode)
  search.handleQueryInput('');
});
