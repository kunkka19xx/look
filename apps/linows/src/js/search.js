import { search as ipcSearch } from './ipc.js';

const DEBOUNCE_MS = 70;
let debounceTimer = null;
let onResultsCallback = null;
let homeDir = null;

const QUICK_FOLDERS = [
  'Desktop', 'Documents', 'Downloads', 'Pictures', 'Videos', 'Music',
];

export function setOnResults(callback) {
  onResultsCallback = callback;
}

export function setHomeDir(home) {
  homeDir = home;
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
    const results = prependQuickFolders(payload.results, query);
    if (onResultsCallback) {
      onResultsCallback(results, query);
    }
  } catch (err) {
    console.error('Search failed:', err);
    if (onResultsCallback) {
      onResultsCallback([], query);
    }
  }
}

function prependQuickFolders(results, query) {
  if (!homeDir) return results;
  const q = query.toLowerCase().trim();
  if (q.length < 2) return results;

  const matched = [];
  for (const name of QUICK_FOLDERS) {
    if (!name.toLowerCase().startsWith(q)) continue;
    const path = `${homeDir}/${name}`;
    // Deduplicate: skip if already in results
    if (results.some((r) => r.path === path)) continue;
    matched.push({
      id: `quickfolder:${name.toLowerCase()}`,
      kind: 'folder',
      title: name,
      subtitle: 'Pinned home folder',
      path,
      score: 999999,
    });
  }

  return [...matched, ...results];
}
