const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export async function search(query, limit = 40) {
  return invoke('search', { query, limit });
}

export async function recordUsage(candidateId, action) {
  return invoke('record_usage', { candidateId, action });
}

export async function openPath(path) {
  return invoke('open_path', { path });
}

export async function revealPath(path) {
  return invoke('reveal_path', { path });
}

export async function reloadConfig() {
  return invoke('reload_config');
}

export async function requestIndexRefresh() {
  return invoke('request_index_refresh');
}

export async function hideWindow() {
  return invoke('hide_window');
}

export async function onWindowShown(callback) {
  return listen('window-shown', callback);
}
