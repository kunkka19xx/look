import { search as ipcSearch } from './ipc.js';

const DEBOUNCE_MS = 70;
let debounceTimer = null;
let onResultsCallback = null;

export function setOnResults(callback) {
  onResultsCallback = callback;
}

export function handleQueryInput(query) {
  clearTimeout(debounceTimer);

  if (query.trim() === '') {
    performSearch('');
    return;
  }

  debounceTimer = setTimeout(() => performSearch(query), DEBOUNCE_MS);
}

async function performSearch(query) {
  try {
    const payload = await ipcSearch(query, 40);
    if (onResultsCallback) {
      onResultsCallback(payload.results, query);
    }
  } catch (err) {
    console.error('Search failed:', err);
    if (onResultsCallback) {
      onResultsCallback([], query);
    }
  }
}
