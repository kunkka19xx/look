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

export async function evalCalc(expr) {
  return invoke('eval_calc', { expr });
}

export async function runShellCommand(cmd) {
  return invoke('run_shell_command', { cmd });
}

export async function getSystemInfo() {
  return invoke('get_system_info');
}

export async function listProcesses() {
  return invoke('list_processes');
}

export async function listProcessesOnPort(port) {
  return invoke('list_processes_on_port', { port });
}

export async function killProcess(pid) {
  return invoke('kill_process', { pid });
}

export async function getHomeDir() {
  return invoke('get_home_dir');
}

export async function scanMusicFolder(folder) {
  return invoke('scan_music_folder', { folder });
}

export async function pickFolder() {
  return invoke('pick_folder');
}

export async function onWindowShown(callback) {
  return listen('window-shown', callback);
}

export async function musicPlay(path) {
  return invoke('music_play', { path });
}

export async function musicPauseBackend() {
  return invoke('music_pause');
}

export async function musicResumeBackend() {
  return invoke('music_resume');
}

export async function musicStopBackend() {
  return invoke('music_stop');
}

export async function musicIsFinished() {
  return invoke('music_is_finished');
}
