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

// Windows only. Rejects when UAC is declined, so await before recording usage.
export async function openElevated(path) {
    return invoke('open_elevated', { path });
}

export async function revealPath(path) {
    return invoke('reveal_path', { path });
}

// Preferred tools (core look-tools). `toolActions` resolves a batch without
// acting, for the menu labels; `performToolAction` resolves one and carries it
// out. Batched because the menu asks about every action at once.
export async function toolActions(actions, path, isDir) {
    return invoke('tool_actions', { actions, path, isDir });
}

export async function performToolAction(action, path, isDir) {
    return invoke('perform_tool_action', { action, path, isDir });
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

/** The armed frame is painted; Rust can drop the window for that dismissal. */
export async function confirmHide(arm) {
    return invoke('confirm_hide', { arm });
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

// Main-field calculator: resolves to null unless the query was clearly meant
// as arithmetic. See core/calc `is_math`.
export async function calcInline(query) {
    return invoke('calc_inline', { query });
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

export async function killProcess(pid) {
    return invoke('kill_process', { pid });
}

export async function searchProcesses(query, refresh) {
    return invoke('search_processes', { query, refresh });
}

export async function searchKillTargets(query) {
    return invoke('search_kill_targets', { query });
}

export async function processDetail(pid) {
    return invoke('process_detail', { pid });
}

export async function processCpu(pid) {
    return invoke('process_cpu', { pid });
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

export async function deleteClipboardEntry(timestamp, text) {
    return invoke('delete_clipboard_entry', { timestamp, text });
}

export async function copyToClipboard(text) {
    return invoke('copy_to_clipboard', { text });
}

// Copies `text`, but files it in clipboard history under `label`. Pasting
// still yields `text`; the history list just reads better.
export async function copyToClipboardLabeled(text, label) {
    return invoke('copy_to_clipboard_labeled', { text, label });
}

export async function resetConfig() {
    return invoke('reset_config');
}

export async function getPlatform() {
    return invoke('get_platform');
}

// Blur region in window-local logical pixels (see platform/linux/blur.rs).
export async function setBlurRegion(rects) {
    return invoke('set_blur_region', { rects });
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

// Send a transport command ('playpause' | 'next' | 'previous') to `player` (the
// handle from its snapshot). Resolves to whether it was delivered.
export async function nowPlayingCommand(command, player) {
    return invoke('now_playing_command', { command, player });
}

// Convert a local calendar date to its lunar date via the shared look-lunar core
// crate. tz is the UTC offset in hours. Returns { day, month, year, leap }.
export async function lunarDate(year, month, day, tz) {
    return invoke('lunar_date', { year, month, day, tz });
}

// Todo: full-set load/save against the shared look-todo store. Tasks are
// `{ id, name, done, due_date, created_at_unix_s }` (same JSON contract as
// the macOS FFI bridge).

// Speed test: one full measurement from the shared look-netspeed crate. Blocks
// for ~15s and up, so the panel drives it as a single in-flight run; rejects
// with core's own message when no direction got through.
export async function speedTest() {
    return invoke('speed_test');
}

// This machine's IPv4 on the local network, or null when only loopback is up.
export async function localIpv4() {
    return invoke('local_ipv4');
}

export async function todoList() {
    return invoke('todo_list');
}

export async function todoSave(tasks) {
    return invoke('todo_save', { tasks });
}

export async function onWindowShown(callback) {
    return listen('window-shown', callback);
}

export async function onWindowHidden(callback) {
    return listen('window-hidden', callback);
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

export async function getInstallMethod() {
    return invoke('get_install_method');
}

export async function startWindowsUpdate(version) {
    return invoke('start_windows_update', { version });
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
