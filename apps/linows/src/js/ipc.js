const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export async function search(query, limit = 40) {
  return invoke('search', { query, limit });
}

export async function recordUsage(candidateId, action) {
  return invoke('record_usage', { candidateId, action });
}

export async function openPath(path, kind, id) {
  return invoke('open_path', { path, kind, id });
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

export async function getIcon(kind, path, id) {
  return invoke('get_icon', { kind, path, id });
}

export async function getFileMeta(path) {
  return invoke('get_file_meta', { path });
}

export async function getAppVersion(path) {
  return invoke('get_app_version', { path });
}

export async function copyFilesToClipboard(paths) {
  return invoke('copy_files_to_clipboard', { paths });
}

export async function getHomeDir() {
  return invoke('get_home_dir');
}

export async function onWindowShown(callback) {
  return listen('window-shown', callback);
}
