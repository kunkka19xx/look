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

export async function forceIndexRefresh() {
  return invoke('force_index_refresh');
}

export async function hideWindow() {
  return invoke('hide_window');
}

export async function quitApp() {
  return invoke('quit_app');
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

export async function isDevBuild() {
  return invoke('is_dev_build');
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

export async function listRunningApps() {
  return invoke('list_running_apps');
}

export async function activateRunningApp(pid, desktopId, exec) {
  return invoke('activate_running_app', { pid, desktopId, exec });
}

export async function getHomeDir() {
  return invoke('get_home_dir');
}

export async function getQuickFolders() {
  return invoke('get_quick_folders');
}

export async function scanMusicFolder(folder) {
  return invoke('scan_music_folder', { folder });
}

export async function pickFolder() {
  return invoke('pick_folder');
}

export async function pickImage() {
  return invoke('pick_image');
}

export async function getClipboardHistory(query = '') {
  return invoke('get_clipboard_history', { query });
}

export async function deleteClipboardEntry(index) {
  return invoke('delete_clipboard_entry', { index });
}

export async function copyToClipboard(text) {
  return invoke('copy_to_clipboard', { text });
}

export async function resetConfig() {
  return invoke('reset_config');
}

export async function getPlatform() {
  return invoke('get_platform');
}

export async function listCandidateDrives() {
  return invoke('list_candidate_drives');
}

export async function setWindowEffect(effect) {
  return invoke('set_window_effect', { effect });
}

export async function listFonts() {
  return invoke('list_fonts');
}

export async function getConfig() {
  return invoke('get_config');
}

export async function setConfig(updates) {
  return invoke('set_config', { updates });
}

export async function translate(text, targetLang) {
  return invoke('translate', { text, targetLang });
}

// Todo: full-set load/save against the shared look-todo store. Tasks are
// `{ id, name, done, due_date, created_at_unix_s }` (same JSON contract as
// the macOS FFI bridge).

export async function todoList() {
  return invoke('todo_list');
}

export async function todoSave(tasks) {
  return invoke('todo_save', { tasks });
}

export async function onWindowShown(callback) {
  return listen('window-shown', callback);
}

export async function onIndexReady(callback) {
  return listen('index-ready', callback);
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

export async function setAutostart(enabled) {
  return invoke('set_autostart', { enabled });
}

export async function getAutostart() {
  return invoke('get_autostart');
}

export async function highlightFile(path) {
  return invoke('highlight_file_cmd', { path });
}

export async function listFolder(path) {
  return invoke('list_folder', { path });
}

export async function getLookappVersion() {
  return invoke('get_lookapp_version');
}

export async function trashPaths(paths) {
  return invoke('trash_paths', { paths });
}

export async function countTrashItems() {
  return invoke('count_trash_items');
}

export async function emptyTrash() {
  return invoke('empty_trash');
}

// AI / web answers: see src-tauri/src/answers.rs. Each returns an Answer
// `{ text, source, url?, image_url? }` or null. The card UI ignores null.

export async function instantHasMatch(query) {
  return invoke('instant_has_match', { query });
}

export async function definitionalEntity(query) {
  return invoke('definitional_entity', { query });
}

export async function instantAnswer(query) {
  return invoke('instant_answer', { query });
}

export async function duckduckgoAnswer(query) {
  return invoke('duckduckgo_answer', { query });
}

export async function wikipediaAnswer(term) {
  return invoke('wikipedia_answer', { term });
}

export async function webSuggestions(query, limit) {
  return invoke('web_suggestions', { query, limit });
}
