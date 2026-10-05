//! linows drawn by gpui-ce. The backend (`linows_backend`) does the work;
//! this binary owns the window, the hotkey, and the control socket.

mod blur;
mod fonts;
#[cfg_attr(target_os = "linux", path = "host/linux.rs")]
#[cfg_attr(windows, path = "host/windows.rs")]
mod host;
mod launcher;
mod motion;
mod search;
mod theme;

use std::borrow::Cow;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use gpui::{
    App, AppContext, AssetSource, QuitMode, SharedString, WindowBounds, WindowOptions, px, size,
};
use linows_backend::health::HealthIssue;
use linows_backend::host::{Host, LauncherWindow};
use linows_backend::look_engine::modes;
use linows_backend::platform::IconCache;
use linows_backend::state::AppState;
use linows_backend::{crash, launch_query};

use launcher::Launcher;

const SEARCH_ICON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.8-3.8"/></svg>"##;

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            "icons/search.svg" => Some(Cow::Borrowed(SEARCH_ICON_SVG)),
            _ => None,
        })
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// What reaches the main loop: from the hotkey, the control socket, and the
/// backend's hooks.
pub enum Command {
    Toggle,
    Show,
    Hide,
    Query(String),
    /// The index finished a refresh; the open query re-runs.
    Refresh,
    Quit,
}

static STATE: OnceLock<AppState> = OnceLock::new();
static ICONS: OnceLock<IconCache> = OnceLock::new();

/// The engine and its watchers, one per process.
pub fn state() -> &'static AppState {
    STATE.get_or_init(AppState::new)
}

pub fn icons() -> &'static IconCache {
    ICONS.get_or_init(IconCache::new)
}

/// Both backend hooks: a sender into the main loop and the visibility flag the
/// open and hide paths keep.
#[derive(Clone)]
struct Shell {
    tx: async_channel::Sender<Command>,
    visible: Arc<AtomicBool>,
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
        for issue in issues {
            eprintln!("[health] {}: {}", issue.id, issue.message);
        }
    }

    /// Owning the clipboard is M1's job (gpui's clipboard for text, wl-copy
    /// for files); until then the backend shells out.
    #[cfg(target_os = "linux")]
    fn own_clipboard(&self, _forms: Vec<linows_backend::host::ClipForm>) -> bool {
        false
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
                    query if query.starts_with("query ") => {
                        Command::Query(query["query ".len()..].to_owned())
                    }
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
    let shell = shell.clone();
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
            cx.new(|cx| Launcher::new(shell, window, cx))
        },
    );
    match result {
        Ok(_) => shell_visible(cx, true),
        Err(err) => eprintln!("open window: {err:#}"),
    }
}

fn hide(cx: &mut App) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
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

fn apply(command: Command, cx: &mut App) {
    match command {
        Command::Query(text) => with_launcher(cx, |launcher, cx| launcher.set_query(&text, cx)),
        Command::Refresh => with_launcher(cx, |launcher, cx| launcher.refresh(cx)),
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
            linows_backend::platform::linux::wayland_shortcut::cleanup_keybinding();
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

    let launch = modes::parse_args(std::env::args().skip(1));
    match &launch {
        modes::Launch::ListModes => {
            print!("{}", modes::list_text());
            return;
        }
        modes::Launch::UnknownMode(name) => {
            eprintln!("lookapp: unknown mode \"{name}\"\n\n{}", modes::list_text());
            std::process::exit(2);
        }
        modes::Launch::UnavailableMode(name) => {
            eprintln!("lookapp: mode \"{name}\" is not available on this platform");
            std::process::exit(2);
        }
        modes::Launch::ReloadConfig => {
            eprintln!("lookapp: reload-config is not wired in the gpui shell yet");
            return;
        }
        // A running launcher answers the same D-Bus call the hotkey makes;
        // with none running, this process becomes it.
        #[cfg(target_os = "linux")]
        modes::Launch::Toggle
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
    };
    linows_backend::host::install(Box::new(shell.clone()));
    state().start_bootstrap();

    if let Err(err) = serve_commands(shell.tx.clone()) {
        eprintln!("control socket: {err}");
    }
    #[cfg(target_os = "linux")]
    {
        linows_backend::platform::linux::prime_user_session();
        let hotkey = shell.clone();
        let bind_key = linows_backend::hotkey::configured().enabled;
        linows_backend::platform::linux::wayland_shortcut::start(bind_key, move || {
            hotkey.send(Command::Toggle);
        });
    }

    gpui_platform::application()
        .with_assets(Assets)
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
