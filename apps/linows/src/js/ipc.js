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

// Quick Actions: descriptors come from the shared look-qactions catalog;
// state/apply go through the native adapter for the action id
// (see docs/writing-controls.md).

export async function quickActions(resultId, kind) {
    return invoke('quick_actions', { resultId, kind });
}

// The empty-state launchpad tile layout, from the shared qactions catalog.
export async function launchpadLayout() {
    return invoke('launchpad_layout');
}

// Compact system uptime ("3d 4h"), shown in the launchpad info tile in place of
// Battery on a machine with no battery. Null when unavailable.
export async function systemUptime() {
    return invoke('system_uptime');
}

export async function quickActionState(actionId, infoKeys) {
    return invoke('quick_action_state', { actionId, infoKeys });
}

export async function quickActionApply(actionId, intent) {
    return invoke('quick_action_apply', { actionId, intent });
}

export async function quickActionApplyItem(actionId, itemId, intent) {
    return invoke('quick_action_apply_item', { actionId, itemId, intent });
}

// Launchpad weather tile: current conditions from the keyless IP-geo + Open-Meteo
// feed, cached in the backend. Null when it can't be resolved (offline first run).
export async function weatherCurrent() {
    return invoke('weather_current');
}

// Launchpad Now Playing tile: the active MPRIS track, or null when nothing plays.
export async function nowPlayingCurrent() {
    return invoke('now_playing_current');
}

// Send a transport command ('playpause' | 'next' | 'previous') to the active
// MPRIS player. Resolves to whether it was delivered.
export async function nowPlayingCommand(command) {
    return invoke('now_playing_command', { command });
}

// Convert a local calendar date to its lunar date via the shared look-lunar core
// crate. tz is the UTC offset in hours. Returns { day, month, year, leap }.
export async function lunarDate(year, month, day, tz) {
    return invoke('lunar_date', { year, month, day, tz });
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

export async function getHealthIssues() {
    return invoke('get_health_issues');
}

export async function onHealthChanged(callback) {
    return listen('health-changed', callback);
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

// URL-like queries + opened-URL history (issue #232 / url-history spec).
// classifyUrl returns `{ url, tier }` or null; recentUrls returns rows
// `{ url, title, hit_count, last_used_at_unix_s, score }` in frecency order.

export async function classifyUrl(query) {
    return invoke('classify_url', { query });
}

export async function recordUrlHit(url) {
    return invoke('record_url_hit', { url });
}

export async function recentUrls(query, limit) {
    return invoke('recent_urls', { query, limit });
}
