//! Every command the webview can invoke, one hop each into `linows_backend`.
//!
//! Nothing is decided here. A command names its arguments the way the frontend
//! spells them, hands them to the backend, and runs the blocking ones on
//! Tauri's pool. The window-bound commands at the end are the exception: the
//! armed hide and the layout resize belong to this shell.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use linows_backend as backend;
use linows_backend::look_answers::{Answer, UrlMatch};
use linows_backend::look_calc::Calculation;
use linows_backend::look_engine::hotkey::HotkeyCheck;
use linows_backend::look_engine::launchpad::{LayoutPayload, TileValue};
use linows_backend::look_engine::sources::{
    BlockDetail, BlockSummary, Level, PerformOutcome, PreviewOutcome, RefreshOutcome,
};
use linows_backend::look_engine::url_history::ScoredUrlEntry;
use linows_backend::look_lunar::LunarDate;
use linows_backend::look_netspeed::SpeedReading;
use linows_backend::look_qactions::ActionDescriptor;
use linows_backend::look_todo::TodoTask;
use linows_backend::look_tools::Resolved;
use serde::Serialize;
use tauri::State;

use backend::clipboard::{ClipboardEntry, ClipboardImageRow};
use backend::config::{ConfigPayload, ConfigUpdate};
use backend::files::{FileMeta, FolderListing, QuickFolder};
use backend::health::HealthIssue;
use backend::highlight::HighlightResult;
use backend::hotkey::LauncherHotkeyState;
use backend::nowplaying::NowPlayingSnapshot;
use backend::platform::{BlurRect, CandidateDrive, IconCache, IconResult};
use backend::process::{KillTarget, ProcDetail, ProcRow, RunningApp};
use backend::qactions::{ActionIntent, ActionOutcome, QuickActionStatus};
use backend::search::{SearchPayload, UsageResult};
use backend::sources::RowArgs;
use backend::state::AppState;
use backend::sysinfo::SysInfoEntry;
use backend::translate::TranslateResult;
use backend::trash::TrashOutcome;
use backend::weather::WeatherSnapshot;

use crate::window::{self, TauriWindow};

/// Blocking backend work, off the request thread. `None` only when the pool
/// task itself died, which each caller maps to its own empty answer.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tauri::async_runtime::spawn_blocking(work).await.ok()
}

/// The pool lost the task before it answered.
const TASK_LOST: &str = "the task did not finish";

// --- Core: search, usage, open, reveal ---

#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String, limit: u32) -> SearchPayload {
    backend::search::search(&state, &query, limit)
}

#[tauri::command]
pub fn record_usage(
    state: State<'_, AppState>,
    candidate_id: String,
    action: String,
) -> UsageResult {
    backend::search::record_usage(&state, &candidate_id, &action)
}

#[tauri::command]
pub fn open_path(
    window: tauri::WebviewWindow,
    path: String,
    kind: Option<String>,
    id: Option<String>,
) -> Result<(), String> {
    backend::launch::open_path(&TauriWindow(window), path, kind.as_deref(), id.as_deref())
}

#[tauri::command]
pub async fn open_elevated(window: tauri::WebviewWindow, path: String) -> Result<(), String> {
    let window = TauriWindow(window);
    blocking(move || backend::launch::open_elevated(&window, &path))
        .await
        .unwrap_or_else(|| Err(TASK_LOST.into()))
}

#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    backend::launch::reveal_path(&path)
}

/// The backend's reload, then the hotkey re-bound in case the config moved it.
#[tauri::command(async)]
pub fn reload_config(app: tauri::AppHandle, state: State<'_, AppState>) -> RefreshOutcome {
    let outcome = backend::search::reload_config(&state);
    crate::launcher_hotkey::set_active(&app, true);
    outcome
}

#[tauri::command]
pub fn request_index_refresh(state: State<'_, AppState>) -> bool {
    state.request_index_refresh()
}

#[tauri::command]
pub fn force_index_refresh(state: State<'_, AppState>) -> bool {
    state.force_index_refresh()
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    eprintln!("look: quit via Alt+Shift+Q");
    app.exit(0);
}

#[tauri::command]
pub fn get_install_method() -> String {
    backend::launch::get_install_method()
}

#[tauri::command]
pub async fn start_windows_update(app: tauri::AppHandle, version: String) -> Result<(), String> {
    let started = blocking(move || backend::launch::start_windows_update(&version))
        .await
        .unwrap_or_else(|| Err(TASK_LOST.into()));
    // The install helper reopens Look once setup ends, whether or not it succeeds.
    if started.is_ok() {
        app.exit(0);
    }
    started
}

// --- Config ---

#[tauri::command]
pub fn get_config() -> ConfigPayload {
    backend::config::get_config()
}

#[tauri::command]
pub fn set_config(updates: Vec<ConfigUpdate>) -> Result<(), String> {
    backend::config::set_config(updates)
}

#[tauri::command]
pub fn reset_config() -> Result<(), String> {
    backend::config::reset_config()
}

#[tauri::command]
pub fn launcher_hotkey_state() -> LauncherHotkeyState {
    backend::hotkey::launcher_hotkey_state()
}

#[tauri::command]
pub fn hotkey_check(spec: String) -> HotkeyCheck {
    backend::hotkey::hotkey_check(&spec)
}

#[tauri::command]
pub fn launcher_hotkey_set_active(app: tauri::AppHandle, active: bool) {
    crate::launcher_hotkey::set_active(&app, active);
}

// --- Files ---

#[tauri::command]
pub fn get_file_meta(path: String) -> FileMeta {
    backend::files::get_file_meta(&path)
}

#[tauri::command]
pub fn get_app_version(path: String) -> Option<String> {
    backend::files::get_app_version(&path)
}

#[tauri::command]
pub fn list_folder(path: String) -> Option<FolderListing> {
    backend::files::list_folder(&path)
}

#[tauri::command]
pub fn is_dev_build() -> bool {
    backend::files::is_dev_build()
}

#[tauri::command]
pub fn copy_files_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    backend::files::copy_files_to_clipboard(&paths)
}

#[tauri::command]
pub fn get_home_dir() -> Option<String> {
    backend::files::get_home_dir()
}

#[tauri::command]
pub fn get_quick_folders() -> Vec<QuickFolder> {
    backend::files::get_quick_folders()
}

#[tauri::command]
pub fn list_fonts() -> Vec<String> {
    backend::files::list_fonts()
}

#[tauri::command]
pub fn scan_music_folder(folder: String) -> Vec<String> {
    backend::files::scan_music_folder(&folder)
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    crate::pickers::pick_folder(&app)
}

#[tauri::command]
pub async fn pick_image(app: tauri::AppHandle) -> Option<String> {
    crate::pickers::pick_image(&app)
}

/// Source of truth is `tauri.conf.json`. Debug builds report a fixed `0.1.0`
/// so the update check can be exercised end-to-end against the latest release.
#[tauri::command]
pub fn get_lookapp_version(app: tauri::AppHandle) -> String {
    if cfg!(debug_assertions) {
        return "0.1.0".to_string();
    }
    app.package_info().version.to_string()
}

// --- Shell ---

#[tauri::command]
pub fn run_shell_command(cmd: String) -> Result<String, String> {
    backend::shell::run_shell_command(&cmd)
}

// --- Preferred tools ---

#[tauri::command(async)]
pub fn tool_actions(
    actions: Vec<String>,
    row: RowArgs,
    is_dir: Option<bool>,
) -> Vec<Option<Resolved>> {
    backend::tools::tool_actions(&actions, row, is_dir)
}

#[tauri::command]
pub async fn perform_tool_action(
    window: tauri::WebviewWindow,
    action: String,
    row: RowArgs,
    is_dir: Option<bool>,
) -> Option<Resolved> {
    let window = TauriWindow(window);
    blocking(move || backend::tools::perform_tool_action(&window, &action, row, is_dir))
        .await
        .flatten()
}

// --- User-declared sources ---

#[tauri::command(async)]
pub fn source_block(row: RowArgs) -> Option<BlockDetail> {
    backend::sources::source_block(row)
}

#[tauri::command(async)]
pub fn source_blocks() -> Vec<BlockSummary> {
    backend::sources::source_blocks()
}

#[tauri::command(async)]
pub fn perform_block(block_id: String, row: RowArgs, as_target: bool) -> PerformOutcome {
    backend::sources::perform_block(block_id, row, as_target)
}

#[tauri::command(async)]
pub fn source_rows(block_id: String, parent: RowArgs) -> Level {
    backend::sources::source_rows(block_id, parent)
}

#[tauri::command(async)]
pub fn source_preview(row: RowArgs) -> Option<PreviewOutcome> {
    backend::sources::source_preview(row)
}

#[tauri::command(async)]
pub fn refresh_run_blocks() -> RefreshOutcome {
    backend::sources::refresh_run_blocks()
}

// --- Platform: icons, detection, drives, window effects ---

#[tauri::command]
pub fn get_icon(
    cache: State<'_, IconCache>,
    kind: String,
    path: String,
    id: Option<String>,
) -> IconResult {
    backend::platform::get_icon(&cache, &kind, &path, id.as_deref())
}

#[derive(Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub has_compositor: bool,
    /// Compositor name when known ("hyprland", "sway", "gnome", "kde", ...).
    /// Exposed to the frontend so CSS can branch on compositor-specific bugs
    /// (e.g. WebKitGTK backdrop-filter glitches on Hyprland).
    pub compositor: Option<String>,
    /// True when a virtual GPU was detected at startup (VM). Hardware
    /// acceleration is already off; the frontend must also drop backdrop
    /// blur or software compositing ghost-renders stale layers.
    pub virtual_gpu: bool,
    /// True when the compositor grants behind-window blur on request. A
    /// capability, not a setting: it only tells the frontend whether Blur
    /// Opacity has real frost to thin.
    pub compositor_blur: bool,
}

#[tauri::command]
pub fn get_platform() -> PlatformInfo {
    let os = std::env::consts::OS.to_string();

    #[cfg(target_os = "linux")]
    let has_compositor = backend::platform::linux::transparency::has_compositor();

    #[cfg(not(target_os = "linux"))]
    let has_compositor = true;

    #[cfg(target_os = "linux")]
    let compositor = backend::platform::linux::wm::detect_compositor();

    #[cfg(not(target_os = "linux"))]
    let compositor: Option<String> = None;

    #[cfg(target_os = "linux")]
    let virtual_gpu = crate::host::gpu::virtual_gpu_detected();

    #[cfg(not(target_os = "linux"))]
    let virtual_gpu = false;

    #[cfg(target_os = "linux")]
    let compositor_blur = crate::host::blur::is_supported();

    // Windows could via DWM acrylic, but it cannot round a per-pixel-alpha
    // window - that trades the rounded silhouette for frost.
    #[cfg(not(target_os = "linux"))]
    let compositor_blur = false;

    PlatformInfo {
        os,
        has_compositor,
        compositor,
        virtual_gpu,
        compositor_blur,
    }
}

#[tauri::command]
pub fn list_candidate_drives() -> Vec<CandidateDrive> {
    backend::platform::list_candidate_drives()
}

#[tauri::command]
pub fn set_window_effect(window: tauri::Window, effect: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        crate::host::effects::apply(window, &effect)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, effect);
        Ok(())
    }
}

// --- Slash commands ---

#[tauri::command]
pub fn eval_calc(expr: String) -> Result<String, String> {
    backend::calc::eval_calc(&expr)
}

#[tauri::command]
pub fn calc_inline(query: String) -> Option<Calculation> {
    backend::calc::calc_inline(&query)
}

#[tauri::command]
pub fn get_system_info() -> Vec<Vec<SysInfoEntry>> {
    backend::sysinfo::get_system_info()
}

#[tauri::command]
pub fn system_uptime() -> Option<String> {
    backend::sysinfo::system_uptime()
}

#[tauri::command]
pub fn list_processes() -> Vec<RunningApp> {
    backend::process::list_processes()
}

#[tauri::command]
pub fn kill_process(pid: u32) -> Result<String, String> {
    backend::process::kill_process(pid)
}

#[tauri::command]
pub async fn search_processes(query: String, refresh: bool) -> Vec<ProcRow> {
    blocking(move || backend::process::search_processes(&query, refresh))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn search_kill_targets(query: String) -> Vec<KillTarget> {
    blocking(move || backend::process::search_kill_targets(&query))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub fn process_detail(pid: u32) -> Option<ProcDetail> {
    backend::process::process_detail(pid)
}

#[tauri::command]
pub async fn process_cpu(pid: u32) -> Option<f64> {
    blocking(move || backend::process::process_cpu(pid))
        .await
        .flatten()
}

#[tauri::command]
pub fn list_running_apps() -> Vec<RunningApp> {
    backend::process::list_running_apps()
}

#[tauri::command]
pub fn activate_running_app(
    window: tauri::WebviewWindow,
    pid: u32,
    desktop_id: Option<String>,
    exec: Option<String>,
) -> Result<bool, String> {
    backend::process::activate_running_app(&TauriWindow(window), pid, desktop_id, exec)
}

// --- Todo ---

#[tauri::command]
pub fn todo_list() -> Result<Vec<TodoTask>, String> {
    backend::todo::todo_list()
}

#[tauri::command]
pub fn todo_save(tasks: Vec<TodoTask>) -> Result<(), String> {
    backend::todo::todo_save(&tasks)
}

// --- Translation ---

#[tauri::command]
pub fn translate(text: String, target_lang: String) -> TranslateResult {
    backend::translate::translate(&text, &target_lang)
}

// --- AI / web answers ---

#[tauri::command]
pub fn instant_has_match(query: String) -> bool {
    backend::answers::instant_has_match(&query)
}

#[tauri::command]
pub fn definitional_entity(query: String) -> Option<String> {
    backend::answers::definitional_entity(&query)
}

#[tauri::command]
pub async fn instant_answer(query: String) -> Option<Answer> {
    blocking(move || backend::answers::instant_answer(&query))
        .await
        .flatten()
}

#[tauri::command]
pub async fn duckduckgo_answer(query: String) -> Option<Answer> {
    blocking(move || backend::answers::duckduckgo_answer(&query))
        .await
        .flatten()
}

#[tauri::command]
pub async fn wikipedia_answer(term: String) -> Option<Answer> {
    blocking(move || backend::answers::wikipedia_answer(&term))
        .await
        .flatten()
}

#[tauri::command]
pub async fn web_suggestions(query: String, limit: usize) -> Vec<String> {
    blocking(move || backend::answers::web_suggestions(&query, limit))
        .await
        .unwrap_or_default()
}

// --- URL-like queries and opened-URL history ---

#[tauri::command]
pub fn classify_url(query: String) -> Option<UrlMatch> {
    backend::weburl::classify_url(&query)
}

#[tauri::command]
pub async fn record_url_hit(url: String) -> bool {
    blocking(move || backend::weburl::record_url_hit(&url))
        .await
        .unwrap_or(false)
}

#[tauri::command]
pub async fn recent_urls(query: String, limit: u32) -> Vec<ScoredUrlEntry> {
    blocking(move || backend::weburl::recent_urls(&query, limit))
        .await
        .unwrap_or_default()
}

// --- Quick actions and the launchpad ---

#[tauri::command]
pub fn quick_actions(result_id: String, kind: String) -> Vec<ActionDescriptor> {
    backend::qactions::quick_actions(&result_id, &kind)
}

#[tauri::command]
pub fn launchpad_layout() -> LayoutPayload {
    backend::qactions::launchpad_layout()
}

#[tauri::command]
pub fn launchpad_tile_values() -> HashMap<String, TileValue> {
    backend::qactions::launchpad_tile_values()
}

#[tauri::command(async)]
pub fn refresh_launchpad_tiles() -> (usize, Vec<String>) {
    backend::qactions::refresh_launchpad_tiles()
}

#[tauri::command(async)]
pub fn press_launchpad_tile(name: String) -> Option<String> {
    backend::qactions::press_launchpad_tile(&name)
}

#[tauri::command]
pub fn launchpad_warnings() -> Vec<String> {
    backend::qactions::launchpad_warnings()
}

#[tauri::command]
pub async fn quick_action_state(action_id: String, info_keys: Vec<String>) -> QuickActionStatus {
    blocking(move || backend::qactions::quick_action_state(&action_id, &info_keys))
        .await
        .unwrap_or_else(backend::qactions::unavailable_status)
}

fn action_lost() -> ActionOutcome {
    ActionOutcome::Failed {
        message: "Action failed".to_string(),
    }
}

#[tauri::command]
pub async fn quick_action_apply(action_id: String, intent: ActionIntent) -> ActionOutcome {
    blocking(move || backend::qactions::quick_action_apply(&action_id, intent))
        .await
        .unwrap_or_else(action_lost)
}

#[tauri::command]
pub async fn quick_action_apply_item(
    action_id: String,
    item_id: String,
    intent: ActionIntent,
) -> ActionOutcome {
    blocking(move || backend::qactions::quick_action_apply_item(&action_id, &item_id, intent))
        .await
        .unwrap_or_else(action_lost)
}

// --- Launchpad feeds ---

#[tauri::command]
pub async fn weather_current() -> Option<WeatherSnapshot> {
    blocking(backend::weather::weather_current).await.flatten()
}

#[tauri::command]
pub async fn now_playing_current() -> Option<NowPlayingSnapshot> {
    blocking(backend::nowplaying::now_playing_current)
        .await
        .flatten()
}

#[tauri::command]
pub async fn now_playing_command(command: String, player: Option<String>) -> bool {
    blocking(move || backend::nowplaying::now_playing_command(&command, player.as_deref()))
        .await
        .unwrap_or(false)
}

#[tauri::command]
pub fn lunar_date(year: i64, month: i64, day: i64, tz: f64) -> LunarDate {
    backend::lunar::lunar_date(year, month, day, tz)
}

#[tauri::command(async)]
pub fn speed_test() -> Result<SpeedReading, String> {
    backend::netspeed::speed_test()
}

#[tauri::command]
pub fn local_ipv4() -> Option<String> {
    backend::netspeed::local_ipv4()
}

// --- Clipboard ---

#[tauri::command]
pub fn get_clipboard_history(query: String) -> Vec<ClipboardEntry> {
    backend::clipboard::get_clipboard_history(&query)
}

#[tauri::command]
pub fn delete_clipboard_entry(timestamp: u64, text: String) -> bool {
    backend::clipboard::delete_clipboard_entry(timestamp, &text)
}

#[tauri::command]
pub fn get_clipboard_images() -> Vec<ClipboardImageRow> {
    backend::clipboard::get_clipboard_images()
}

#[tauri::command]
pub fn delete_clipboard_image(hash: String) -> bool {
    backend::clipboard::delete_clipboard_image(&hash)
}

#[tauri::command]
pub fn clipboard_image_data_url(hash: String) -> Option<String> {
    backend::clipboard::clipboard_image_data_url(&hash)
}

#[tauri::command]
pub fn copy_clipboard_image(hash: String) -> Result<(), String> {
    backend::clipboard::copy_clipboard_image(hash)
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    backend::clipboard::copy_to_clipboard(&text)
}

#[tauri::command]
pub fn copy_to_clipboard_labeled(text: String, label: String) -> Result<(), String> {
    backend::clipboard::copy_to_clipboard_labeled(text, label)
}

#[tauri::command]
pub fn clipboard_paste_blocker() -> Option<String> {
    backend::paste::clipboard_paste_blocker()
}

#[tauri::command]
pub fn paste_into_focused_app(window: tauri::WebviewWindow) {
    backend::paste::paste_into_focused_app(Arc::new(TauriWindow(window)));
}

// --- Music ---

#[tauri::command]
pub fn music_play(path: String) -> Result<(), String> {
    backend::music::music_play(&path)
}

#[tauri::command]
pub fn music_pause() {
    backend::music::music_pause();
}

#[tauri::command]
pub fn music_resume() {
    backend::music::music_resume();
}

#[tauri::command]
pub fn music_stop() {
    backend::music::music_stop();
}

#[tauri::command]
pub fn music_is_finished() -> bool {
    backend::music::music_is_finished()
}

// --- Trash ---

#[tauri::command]
pub fn trash_paths(paths: Vec<String>) -> TrashOutcome {
    backend::trash::trash_paths(paths)
}

#[tauri::command]
pub fn count_trash_items() -> Result<usize, String> {
    backend::trash::count_trash_items()
}

#[tauri::command]
pub fn empty_trash() -> Result<usize, String> {
    backend::trash::empty_trash()
}

// --- Autostart and PATH ---

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    backend::autostart::set_autostart(enabled)
}

#[tauri::command]
pub fn get_autostart() -> bool {
    backend::autostart::get_autostart()
}

#[tauri::command]
pub fn set_cli_path(enabled: bool) -> Result<(), String> {
    backend::cli_path::set_cli_path(enabled)
}

#[tauri::command]
pub fn get_cli_path() -> bool {
    backend::cli_path::get_cli_path()
}

// --- Setup health ---

#[tauri::command]
pub fn get_health_issues() -> Vec<HealthIssue> {
    backend::health::get_health_issues()
}

// --- Highlight ---

#[tauri::command]
pub fn highlight_file_cmd(path: String) -> Option<HighlightResult> {
    backend::highlight::highlight_file_cmd(&path)
}

#[tauri::command]
pub fn highlight_shell_cmd(source: String) -> HighlightResult {
    backend::highlight::highlight_shell_cmd(&source)
}

// --- The window itself ---

#[tauri::command]
pub fn toggle_window(window: tauri::WebviewWindow) {
    if window::launcher_visible(&window) {
        window::hide_armed(&window);
    } else {
        window::show_launcher(&window);
        window::focus_launcher(&window);
    }
}

#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) {
    window::hide_armed(&window);
}

/// `NonZeroU64` keeps the idle sentinel out of the compare: a payload of 0 is
/// rejected while deserializing, never as a match against an idle arm.
#[tauri::command]
pub fn confirm_hide(window: tauri::WebviewWindow, arm: NonZeroU64) {
    window::confirm_hide(&window, arm.get());
}

#[tauri::command]
pub fn take_launch_query() -> Option<String> {
    backend::launch_query::take_launch_query()
}

/// Blur behind the surfaces the frontend paints, in logical pixels. A no-op
/// wherever the compositor has no such request (see host::blur).
#[tauri::command]
pub fn set_blur_region(
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] window: tauri::WebviewWindow,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] rects: Vec<BlurRect>,
) {
    // No X11 id on native Wayland; that path addresses the bound surface.
    #[cfg(target_os = "linux")]
    {
        let wid = backend::platform::linux::window_focus::self_window();
        let scale = window.scale_factor().unwrap_or(1.0);
        crate::host::blur::set_region(wid, &rects, scale);
    }
}

/// Resizes for a layout change, keeping a visible window's top edge and centre.
/// `session_layout` is the Ctrl+Shift+C override, `None` to follow the config.
/// A hidden window is resized by `recenter_window` on its next show; the layer
/// surface is not, so it is resized here either way.
#[tauri::command]
pub fn apply_layout(window: tauri::WebviewWindow, session_layout: Option<String>) {
    backend::geometry::set_session_layout(
        session_layout
            .as_deref()
            .and_then(backend::config::LauncherLayout::parse),
    );
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| crate::monitor_at_cursor(&window))
    else {
        return;
    };
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = backend::geometry::scaled_window_size(screen.height, scale);

    #[cfg(target_os = "linux")]
    if crate::host::layer_shell::is_active() {
        crate::host::layer_shell::resize(win_w as i32, win_h as i32);
        return;
    }

    if !window::launcher_visible(&window) {
        return;
    }
    let (Ok(position), Ok(old_size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let top = position.y as f64 / scale;
    let center_x = (position.x as f64 + old_size.width as f64 / 2.0) / scale;
    crate::resize_locked(&window, tauri::LogicalSize::new(win_w as f64, win_h as f64));
    let left = center_x - win_w as f64 / 2.0;
    let _ = window.set_position(tauri::LogicalPosition::new(left, top));
}
