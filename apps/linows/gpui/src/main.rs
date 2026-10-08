//! linows drawn by gpui-ce. The backend (`linows_backend`) does the work;
//! this binary owns the window, the hotkey, and the control socket.

mod actions;
mod answers;
mod banner;
mod bg;
mod bgimage;
mod blocks;
mod blur;
mod commands;
mod config_list;
mod confirm;
mod controls;
mod elide;
mod fonts;
mod glyphs;
mod health;
mod help;
#[cfg_attr(target_os = "linux", path = "host/linux.rs")]
#[cfg_attr(windows, path = "host/windows.rs")]
mod host;
mod icons;
mod launcher;
mod launchpad;
mod levels;
mod modes;
mod motion;
mod pick;
mod picked;
mod pomo;
mod preview;
mod query;
mod rows;
mod running;
mod search;
mod settings;
mod shortcuts;
mod speed;
mod theme;
mod todo;
mod update;

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use gpui::{
    App, AppContext, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, QuitMode, ScrollDelta, ScrollWheelEvent, TouchPhase, WindowBounds,
    WindowOptions, point, px, size,
};
use linows_backend::health::HealthIssue;
use linows_backend::host::{ClipForm, Host, LauncherWindow};
use linows_backend::look_engine::modes as engine_modes;
use linows_backend::platform::IconCache;
use linows_backend::query_retention;
use linows_backend::state::AppState;
use linows_backend::{crash, launch_query};

use launcher::{Launcher, Retained};

/// What reaches the main loop: from the hotkey, the control socket, and the
/// backend's hooks.
pub enum Command {
    Toggle,
    Show,
    Hide,
    Query(String),
    /// Dev control: a keystroke handed to the launcher, so a probe can reach
    /// what a key reaches without synthesizing one at the compositor.
    Key(gpui::Keystroke),
    /// Dev control: a left button press and release, through the window's
    /// own dispatch, for what only a click reaches. Drag moves between the
    /// press and the release.
    Click(Point<Pixels>),
    Drag(Point<Pixels>, Point<Pixels>),
    /// Dev control: a wheel turn at a point, in lines, for what scrolls.
    Wheel(Point<Pixels>, f32),
    /// The index finished a refresh; the open query re-runs.
    Refresh,
    /// The backend's setup problems changed; the sticky notice follows.
    Health(Vec<HealthIssue>),
    /// The backend wants the clipboard owned for these forms; the reply says
    /// whether the shell took it.
    OwnClipboard(Vec<ClipForm>, std::sync::mpsc::SyncSender<bool>),
    Quit,
}

/// How long the backend waits for the main loop's clipboard answer before it
/// shells out instead.
const CLIPBOARD_REPLY_TIMEOUT: Duration = Duration::from_secs(2);
/// The spelling a text form carries first, see the backend's TEXT_TARGETS.
const TEXT_TARGET: &str = "text/plain;charset=utf-8";

/// `LOOK_PACE_PROBE=1`: a measurement instance. Logs a timestamp per frame
/// and the search timings for tools/pace.sh, and keeps off the hotkey.
pub fn probe_wanted() -> bool {
    std::env::var_os("LOOK_PACE_PROBE").is_some()
}

static STATE: OnceLock<AppState> = OnceLock::new();
static ICONS: OnceLock<IconCache> = OnceLock::new();

/// The engine and its watchers, one per process.
pub fn state() -> &'static AppState {
    STATE.get_or_init(AppState::new)
}

pub fn icon_cache() -> &'static IconCache {
    ICONS.get_or_init(IconCache::new)
}

/// Both backend hooks: a sender into the main loop and the visibility flag the
/// open and hide paths keep.
#[derive(Clone)]
pub struct Shell {
    tx: async_channel::Sender<Command>,
    visible: Arc<AtomicBool>,
    /// What the last hide left behind, for the retention rule to restore
    /// or drop on the next summon.
    retained: Arc<Mutex<Retained>>,
}

impl Shell {
    fn send(&self, command: Command) {
        let _ = self.tx.send_blocking(command);
    }
}

impl Host for Shell {
    fn index_ready(&self) {
        self.send(Command::Refresh);
    }

    fn health_changed(&self, issues: Vec<HealthIssue>) {
        self.send(Command::Health(issues));
    }

    /// Text goes through gpui's clipboard on the main loop. A file or image
    /// copy needs its MIME types offered side by side, which gpui cannot do,
    /// so those answer `false` and the backend shells out to wl-copy.
    #[cfg(target_os = "linux")]
    fn own_clipboard(&self, forms: Vec<ClipForm>) -> bool {
        if !forms.iter().all(|form| form.targets.contains(&TEXT_TARGET)) {
            return false;
        }
        let (reply, wait) = std::sync::mpsc::sync_channel(1);
        self.send(Command::OwnClipboard(forms, reply));
        wait.recv_timeout(CLIPBOARD_REPLY_TIMEOUT).unwrap_or(false)
    }
}

impl LauncherWindow for Shell {
    fn hide(&self) {
        self.send(Command::Hide);
    }

    fn hide_now(&self) {
        self.send(Command::Hide);
    }

    fn show(&self) {
        self.send(Command::Show);
    }

    /// A layer surface takes the keyboard on its own; the popup is focused at open.
    fn focus(&self) {}

    fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Relaxed)
    }
}

/// Dev control, standing in for a D-Bus call that carries text: `query <text>`
/// fills the field so a results screenshot needs no typing.
fn serve_commands(tx: async_channel::Sender<Command>) -> std::io::Result<()> {
    let listener = host::bind()?;
    std::thread::Builder::new()
        .name("look-control".into())
        .spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut text = String::new();
                if stream.read_to_string(&mut text).is_err() {
                    continue;
                }
                let command = match text.trim() {
                    "toggle" => Command::Toggle,
                    "show" => Command::Show,
                    "hide" => Command::Hide,
                    "quit" => Command::Quit,
                    key if key.starts_with("key ") => {
                        match gpui::Keystroke::parse(key["key ".len()..].trim()) {
                            Ok(keystroke) => Command::Key(keystroke),
                            Err(err) => {
                                eprintln!("bad keystroke: {err}");
                                continue;
                            }
                        }
                    }
                    query if query.starts_with("query ") => {
                        Command::Query(query["query ".len()..].to_owned())
                    }
                    click if click.starts_with("click ") => {
                        match points(&click["click ".len()..])[..] {
                            [at] => Command::Click(at),
                            _ => {
                                eprintln!("usage: click <x> <y>");
                                continue;
                            }
                        }
                    }
                    wheel if wheel.starts_with("wheel ") => {
                        let numbers: Vec<f32> = wheel["wheel ".len()..]
                            .split_whitespace()
                            .filter_map(|n| n.parse().ok())
                            .collect();
                        match numbers[..] {
                            [x, y, lines] => Command::Wheel(point(px(x), px(y)), lines),
                            _ => {
                                eprintln!("usage: wheel <x> <y> <lines>");
                                continue;
                            }
                        }
                    }
                    drag if drag.starts_with("drag ") => match points(&drag["drag ".len()..])[..] {
                        [from, to] => Command::Drag(from, to),
                        _ => {
                            eprintln!("usage: drag <x1> <y1> <x2> <y2>");
                            continue;
                        }
                    },
                    other => {
                        eprintln!("unknown command {other:?}");
                        continue;
                    }
                };
                if tx.send_blocking(command).is_err() {
                    break;
                }
            }
        })?;
    Ok(())
}

fn open(shell: &Shell, cx: &mut App) {
    if !cx.windows().is_empty() {
        return;
    }
    // What a summon does on every shell: the config may have changed, and so
    // may the files.
    theme::load();
    fonts::ensure_family(cx, &theme::get().font_family);
    state().request_index_refresh();
    let for_launcher = shell.clone();
    let bounds = host::bounds(size(px(theme::WINDOW_W), px(theme::WINDOW_H)), cx);
    let result = cx.open_window(
        WindowOptions {
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            app_id: Some("lookapp".into()),
            window_background: host::BACKGROUND,
            kind: host::kind(),
            focus: true,
            show: true,
            is_movable: false,
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            host::decorate(window);
            cx.new(|cx| Launcher::new(for_launcher, window, cx))
        },
    );
    match result {
        Ok(_) => {
            shell_visible(cx, true);
            // A command screen stays up until Esc leaves it, however long the
            // launcher was away; the query alone is subject to the retention
            // window.
            let expired = query_retention::query_clear_decision_after_show(true) != Some(false);
            let mut kept =
                std::mem::take(&mut *shell.retained.lock().unwrap_or_else(|p| p.into_inner()));
            if expired {
                kept.query.clear();
            }
            with_launcher(cx, |launcher, cx| launcher.restore(&kept, cx));
        }
        Err(err) => eprintln!("open window: {err:#}"),
    }
}

fn hide(cx: &mut App) {
    let shell = cx.global::<Shell>().clone();
    for handle in cx.windows() {
        let _ = handle.update(cx, |view, window, cx| {
            if let Ok(launcher) = view.downcast::<Launcher>() {
                *shell.retained.lock().unwrap_or_else(|p| p.into_inner()) =
                    launcher.read(cx).retained(cx);
            }
            window.remove_window();
        });
    }
    query_retention::mark_hidden_now();
    shell_visible(cx, false);
}

fn shell_visible(cx: &mut App, visible: bool) {
    cx.global::<Shell>()
        .visible
        .store(visible, Ordering::Relaxed);
}

impl gpui::Global for Shell {}

fn with_launcher(cx: &mut App, f: impl Fn(&mut Launcher, &mut gpui::Context<Launcher>)) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |view, _, cx| {
            if let Ok(launcher) = view.downcast::<Launcher>() {
                launcher.update(cx, |launcher, cx| f(launcher, cx));
            }
        });
    }
}

/// Window pixels, pairwise, from "x y x y ...".
fn points(text: &str) -> Vec<Point<Pixels>> {
    let numbers: Vec<f32> = text
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    numbers
        .chunks_exact(2)
        .map(|pair| point(px(pair[0]), px(pair[1])))
        .collect()
}

/// Press at the first point, move through the rest, release at the last.
fn wheel(cx: &mut App, at: Point<Pixels>, lines: f32) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position: at,
                    delta: ScrollDelta::Lines(point(0.0, -lines)),
                    modifiers: Modifiers::default(),
                    touch_phase: TouchPhase::Moved,
                }),
                cx,
            );
        });
    }
}

fn mouse(cx: &mut App, path: &[Point<Pixels>]) {
    let (Some(first), Some(last)) = (path.first(), path.last()) else {
        return;
    };
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, cx| {
            let modifiers = Modifiers::default();
            let moved = |position, pressed_button| {
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button,
                    modifiers,
                })
            };
            window.dispatch_event(moved(*first, None), cx);
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: *first,
                    modifiers,
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            for at in &path[1..] {
                window.dispatch_event(moved(*at, Some(MouseButton::Left)), cx);
            }
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: *last,
                    modifiers,
                    click_count: 1,
                }),
                cx,
            );
        });
    }
}

fn apply(command: Command, cx: &mut App) {
    match command {
        Command::Query(text) => with_launcher(cx, |launcher, cx| launcher.set_query(&text, cx)),
        Command::Key(keystroke) => {
            with_launcher(cx, |launcher, cx| launcher.press_key(keystroke.clone(), cx))
        }
        Command::Click(at) => mouse(cx, &[at, at]),
        Command::Drag(from, to) => mouse(cx, &[from, to]),
        Command::Wheel(at, lines) => wheel(cx, at, lines),
        Command::Refresh => with_launcher(cx, |launcher, cx| launcher.refresh(cx)),
        Command::Health(issues) => {
            with_launcher(cx, |launcher, cx| launcher.set_health(issues.clone(), cx))
        }
        Command::OwnClipboard(forms, reply) => {
            let text = forms
                .into_iter()
                .find(|form| form.targets.contains(&TEXT_TARGET))
                .and_then(|form| String::from_utf8(form.payload).ok());
            let taken = match text {
                Some(text) => {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                    true
                }
                None => false,
            };
            let _ = reply.send(taken);
        }
        Command::Toggle if cx.windows().is_empty() => {
            let shell = cx.global::<Shell>().clone();
            open(&shell, cx);
        }
        Command::Toggle | Command::Hide => hide(cx),
        Command::Show => {
            let shell = cx.global::<Shell>().clone();
            open(&shell, cx);
        }
        Command::Quit => {
            // The compositor keybinding this process registered goes with it,
            // as the Tauri shell does on exit.
            #[cfg(target_os = "linux")]
            if !probe_wanted() {
                linows_backend::platform::linux::wayland_shortcut::cleanup_keybinding();
            }
            cx.quit()
        }
    }
}

fn main() {
    crash::install_panic_hook();

    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("lookapp-gpui {}", linows_backend::APP_VERSION);
        return;
    }

    let launch = engine_modes::parse_args(std::env::args().skip(1));
    match &launch {
        engine_modes::Launch::ListModes => {
            print!("{}", engine_modes::list_text());
            return;
        }
        engine_modes::Launch::UnknownMode(name) => {
            eprintln!(
                "lookapp: unknown mode \"{name}\"\n\n{}",
                engine_modes::list_text()
            );
            std::process::exit(2);
        }
        engine_modes::Launch::UnavailableMode(name) => {
            eprintln!("lookapp: mode \"{name}\" is not available on this platform");
            std::process::exit(2);
        }
        engine_modes::Launch::ReloadConfig => {
            eprintln!("lookapp: reload-config is not wired in the gpui shell yet");
            return;
        }
        // A running launcher answers the same D-Bus call the hotkey makes;
        // with none running, this process becomes it.
        #[cfg(target_os = "linux")]
        engine_modes::Launch::Toggle
            if linows_backend::platform::linux::wayland_shortcut::request_toggle() =>
        {
            return;
        }
        _ => {}
    }

    #[cfg(debug_assertions)]
    linows_backend::startup::setup_dev_env();
    // Autostart and PATH registration (`startup::sync_integrations`) stay with
    // the shipping shell until this one replaces it.

    let start_hidden = std::env::args().any(|a| a == "--hidden");
    let (tx, rx) = async_channel::unbounded();
    let shell = Shell {
        tx,
        visible: Arc::new(AtomicBool::new(false)),
        retained: Arc::new(Mutex::new(Retained::default())),
    };
    linows_backend::host::install(Box::new(shell.clone()));
    state().start_bootstrap();
    // The histories `c"` and `ci"` list: loaded from disk now, then kept by
    // the poll thread.
    linows_backend::clipboard::start_monitor();

    if let Err(err) = serve_commands(shell.tx.clone()) {
        eprintln!("control socket: {err}");
    }
    #[cfg(target_os = "linux")]
    {
        linows_backend::platform::linux::prime_user_session();
        // A measurement instance (tools/pace.sh) runs beside the real one and
        // must not take its key or D-Bus name.
        if !probe_wanted() {
            let hotkey = shell.clone();
            let bind_key = linows_backend::hotkey::configured().enabled;
            linows_backend::platform::linux::wayland_shortcut::start(bind_key, move || {
                hotkey.send(Command::Toggle);
            });
        }
    }

    motion::set_desktop_reduces(linows_backend::platform::reduce_motion());
    gpui_platform::application()
        .with_assets(glyphs::Assets)
        .run(move |cx: &mut App| {
            // Hiding the launcher closes its only window; the process stays.
            cx.set_quit_mode(QuitMode::Explicit);
            cx.set_global(shell.clone());
            cx.spawn(async move |cx| {
                while let Ok(command) = rx.recv().await {
                    cx.update(|cx| apply(command, cx));
                }
            })
            .detach();
            if !start_hidden {
                launch_query::park_launch(&launch);
                open(&shell, cx);
            }
        });
}
