//! The launcher shell: the banner slot, the search bar, the launchpad bento
//! while the query is empty, and the results card once it is not, with the
//! preview beside it in the split layout. A prefix puts it in a mode
//! (`modes.rs`) that owns the list, the keys and the footer. Floating
//! layout: no window box, the bar and the cards float on the desktop with
//! the inner gap as every seam.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, ClipboardItem, Context, Div, Entity, FontWeight,
    KeyDownEvent, Pixels, Render, ScrollStrategy, SharedString, Task, UniformListScrollHandle,
    Window, div, img, prelude::*, px, svg, uniform_list,
};
use linows_backend::health::HealthIssue;
use linows_backend::host::LauncherWindow;
use linows_backend::search as engine;
use linows_backend::{
    clipboard, config, files, launch, paste, process, sources, tools, translate, trash, weburl,
};

use crate::actions::{self, ActionId, Menu};
use crate::answers::{self, AiAnswer};
use crate::banner::{self, Banner, Message};
use crate::bg;
use crate::blocks;
use crate::blur::{self, BlurRect};
use crate::confirm::{Confirm, OnYes};
use crate::glyphs;
use crate::icons::{IconRequest, IconStore};
use crate::launchpad::{Launchpad, Notice, Tone};
use crate::levels::{self, Levels};
use crate::modes::Mode;
use crate::motion;
use crate::picked::{self, Picked};
use crate::preview::{ClipDeleted, Preview};
use crate::query;
use crate::rows::{Icon, Open, Row};
use crate::running::RunningApps;
use crate::search::{Changed, SearchInput, search_field};
use crate::theme::{self, Theme};
use crate::{Shell, health, state as app_state};

const PLACEHOLDER: &str = "Search apps, files, actions";
const HINT_EMPTY: &str = "No match \u{2022} Ctrl+Enter: Search the web";
const HINT_COMPOSING: &str = "Composing with fcitx5";
const WEB_SEARCH_URL: &str = "https://www.google.com/search?q=";
const TRANSLATE_URL: &str = "https://translate.google.com/?sl=auto&tl=en&text=";
/// How long a keystroke waits for the next before the query runs; the
/// webview uses the same.
const DEBOUNCE: Duration = Duration::from_millis(70);
/// A row's picture, or the glyph that stands in for it.
const ROW_ICON: f32 = 22.0;
const EMPTY_STATE_H: f32 = 200.0;
const EMPTY_STATE_ICON: f32 = 36.0;
const EMPTY_STATE_PADDING_X: f32 = 24.0;
const EMPTY_STATE_PADDING_Y: f32 = 32.0;
const EMPTY_STATE_GAP: f32 = 6.0;
const EMPTY_HELP_MAX_W: f32 = 360.0;
const NO_RESULTS: &str = "No results";
const TRANSLATE_PADDING_Y: f32 = 10.0;
const TRANSLATE_SECTION_PADDING_Y: f32 = 8.0;
const TRANSLATE_ICON: f32 = 44.0;
const TRANSLATE_BADGE: &str = "WEB";
const TRANSLATE_PLACEHOLDER: &str = "Press Enter to translate";
const TRANSLATING: &str = "Translating\u{2026}";
const TRANSLATE_FAILED: &str = "Translation failed";
const OPEN_IN_BROWSER: &str = "Open in Browser";
/// The three targets, in the panel's order.
const LANGUAGES: [(&str, &str); 3] = [("vi", "TIẾNG VIỆT"), ("en", "ENGLISH"), ("ja", "日本語")];
const CLIP_DELETED: &str = "Clipboard item deleted";
const CLIP_IMAGE_DELETED: &str = "Image removed from history";
const COMMANDS_PENDING: &str = "Command screens arrive with the next milestone";
const HINT_LEVEL: &str = "Enter: Open \u{2022} Ctrl+K: Actions \u{2022} Esc: Back";
const BREADCRUMB_SEPARATOR: &str = "  \u{203a}  ";
const WEB_SUGGESTIONS_LIMIT: usize = 6;
const MIN_WEB_SUGGESTION_QUERY: usize = 2;
const TRASH_LABEL: &str = "Trash";
/// The quick-folder pin for the OS trash: `Trash` here, `Recycle Bin` on
/// Windows.
const TRASH_PIN_IDS: [&str; 2] = ["quickfolder:trash", "quickfolder:recycle bin"];
const APP_EXCLUDE_KEY: &str = "app_exclude_names";
/// `look_indexing::UsageAction::EXECUTE`: a block run ranks like an open.
const USAGE_EXECUTE: &str = "execute";

/// One translation request and what has come back for each language.
struct Translation {
    text: String,
    /// None while in flight.
    sections: Vec<Option<Result<String, String>>>,
}

pub struct Launcher {
    pub(crate) shell: Shell,
    input: Entity<SearchInput>,
    icons: Entity<IconStore>,
    preview: Entity<Preview>,
    launchpad: Entity<Launchpad>,
    pub(crate) banner: Banner,
    health: Vec<HealthIssue>,
    rows: Arc<Vec<Row>>,
    selected: usize,
    scroll: UniformListScrollHandle,
    mode: Mode,
    /// Walk `/proc` again on the next `ps"` search: on entering the mode and
    /// after a kill.
    refresh_processes: bool,
    translation: Option<Translation>,
    picked: Picked,
    confirm: Option<Confirm>,
    menu: Option<Menu>,
    pub(crate) running: RunningApps,
    pub(crate) ai: AiAnswer,
    levels: Levels,
    /// A selection to put back once the rows it names are on screen, and
    /// the query it was captured under: a level was just left.
    pending_restore: Option<(String, String)>,
    /// Bumped on every menu open and close, so a list still resolving when
    /// the user moved on cannot open behind them.
    menu_token: u64,
    /// Bumped per keystroke; a search that comes back for an older one is dropped.
    version: u64,
    _search: Option<Task<()>>,
}

impl Launcher {
    pub fn new(shell: Shell, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(SearchInput::new);
        cx.subscribe(&input, |this, _, _: &Changed, cx| this.search(cx))
            .detach();
        let icons = cx.new(|_| IconStore::new());
        cx.observe(&icons, |_, _, cx| cx.notify()).detach();
        let preview = cx.new(|_| Preview::new(shell.clone(), icons.clone()));
        cx.observe(&preview, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&preview, |this, _, _: &ClipDeleted, cx| {
            this.clip_removed(cx);
        })
        .detach();
        let launchpad = cx.new(Launchpad::new);
        cx.observe(&launchpad, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&launchpad, |this, _, notice: &Notice, cx| {
            this.banner
                .show(notice.text.clone(), notice.tone, notice.seconds, cx)
        })
        .detach();
        let focus_handle = input.read(cx).focus_handle.clone();
        window.focus(&focus_handle, cx);
        blur::attach_window(window);

        let mut this = Self {
            shell,
            input,
            icons,
            preview,
            launchpad,
            banner: Banner::default(),
            health: Vec::new(),
            rows: Arc::new(Vec::new()),
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            mode: Mode::Search,
            refresh_processes: false,
            translation: None,
            picked: Picked::default(),
            confirm: None,
            menu: None,
            running: RunningApps::default(),
            ai: AiAnswer::default(),
            levels: Levels::default(),
            pending_restore: None,
            menu_token: 0,
            version: 0,
            _search: None,
        };
        // Issues reported before this window existed.
        this.set_health(linows_backend::health::get_health_issues(), cx);
        this.running.refresh(cx);
        // The blocks' names and icons, and whether the web answers are on.
        bg::fetch(
            cx,
            || {
                blocks::refresh();
                config::get_config()
                    .entries
                    .into_iter()
                    .find(|e| e.key == answers::AI_KEY)
                    .is_none_or(|e| e.value != "false")
            },
            |this, ai_enabled, _| this.ai.enabled = ai_enabled,
        );
        this
    }

    pub fn query(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text().to_string()
    }

    pub fn set_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// The query a short dismissal left behind, selected whole so the next
    /// keystroke replaces it and Right edits it.
    pub fn restore_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| {
            input.set_text(text, cx);
            input.select_all(cx);
        });
    }

    /// The control socket's keystroke, through the same handler a real one
    /// reaches.
    pub fn press_key(&mut self, keystroke: gpui::Keystroke, cx: &mut Context<Self>) {
        self.handle_key(&keystroke, cx);
    }

    /// Run the query again: a keystroke, or the index finishing a refresh.
    /// The rows come back off the UI thread; a stale answer is dropped.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.search(cx);
    }

    /// The backend's setup problems: the sticky notice shows the ones not
    /// yet dismissed.
    pub fn set_health(&mut self, issues: Vec<HealthIssue>, cx: &mut Context<Self>) {
        self.banner
            .set_sticky(health::notice(&issues).map(|text| Message {
                text,
                tone: Tone::Warning,
            }));
        self.health = issues;
        cx.notify();
    }

    pub(crate) fn dismiss_health(&mut self, cx: &mut Context<Self>) {
        health::dismiss(&self.health);
        self.banner.set_sticky(None);
        cx.notify();
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        self.version += 1;
        let version = self.version;
        let query = self.input.read(cx).committed();
        // A level owns the list: its rows are produced live and are not in
        // the index, so typing filters them rather than searching.
        if self.levels.is_active() {
            self.mode = Mode::Search;
            self.close_menu();
            self.rows = Arc::new(self.levels.rows(&query));
            self.selected = 0;
            self.sync_preview(cx);
            cx.notify();
            return;
        }
        let home = query.trim().is_empty();
        let (mode, _) = Mode::of(&query);
        if mode != Mode::Search || home {
            self.ai.cancel();
        }
        let refresh = self.refresh_processes || (mode == Mode::Process && self.mode != mode);
        self.refresh_processes = false;
        if mode != Mode::Translate {
            self.translation = None;
        }
        self.mode = mode;
        self.close_menu();
        self.launchpad
            .update(cx, |launchpad, cx| launchpad.set_shown(home, cx));
        if home {
            self.rows = Arc::new(Vec::new());
            self.selected = 0;
            self.sync_preview(cx);
            cx.notify();
            return;
        }
        self._search = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let typed = query.clone();
            let rows = cx
                .background_executor()
                .spawn(async move {
                    let query = typed;
                    let started = std::time::Instant::now();
                    let rows = query::run(&query, refresh);
                    if crate::probe_wanted() {
                        eprintln!(
                            "search {:.1} ms rows={} query={query:?}",
                            started.elapsed().as_secs_f64() * 1000.0,
                            rows.len()
                        );
                    }
                    rows
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.version != version {
                    return;
                }
                let local = rows.len();
                this.rows = Arc::new(rows);
                this.selected = 0;
                if let Some((for_query, id)) = this.pending_restore.take()
                    && for_query == query
                    && let Some(at) = this.rows.iter().position(|r| r.id == id)
                {
                    this.selected = at;
                }
                this.scroll
                    .scroll_to_item(this.selected, ScrollStrategy::Top);
                this.sync_preview(cx);
                if mode == Mode::Search && !query.trim_start().contains('"') {
                    this.ai.update(&query, local, cx);
                    this.fetch_web_suggestions(query, version, cx);
                } else {
                    this.ai.cancel();
                }
                cx.notify();
            });
        }));
    }

    /// Google autocomplete rows under the local ones, when the answers are
    /// on and the query is a word or more. Best effort: an empty answer
    /// keeps the list as it is.
    fn fetch_web_suggestions(&mut self, query: String, version: u64, cx: &mut Context<Self>) {
        let trimmed = query.trim().to_string();
        if !self.ai.enabled || trimmed.len() < MIN_WEB_SUGGESTION_QUERY {
            return;
        }
        bg::fetch(
            cx,
            move || linows_backend::answers::web_suggestions(&trimmed, WEB_SUGGESTIONS_LIMIT),
            move |this, list, cx| {
                if this.version != version || list.is_empty() || this.levels.is_active() {
                    return;
                }
                let mut rows: Vec<Row> = this
                    .rows
                    .iter()
                    .filter(|r| !matches!(r.open, Open::WebSuggestion(_)))
                    .cloned()
                    .collect();
                rows.extend(
                    list.iter()
                        .enumerate()
                        .map(|(i, text)| Row::web_suggestion(text, i)),
                );
                this.rows = Arc::new(rows);
                this.sync_preview(cx);
            },
        );
    }

    // --- Levels ----------------------------------------------------------------

    /// Open `block_id` as a level below `parent`, which is passed in rather
    /// than read from the selection: the user may have moved on.
    fn descend(&mut self, block_id: String, title: String, parent: Row, cx: &mut Context<Self>) {
        let token = self.levels.begin();
        let restored_query = self.input.read(cx).committed();
        let restored_selection = Some(parent.id.clone());
        let args = sources::RowArgs {
            candidate_id: parent.id.clone(),
            row_title: parent.title.clone(),
            row_path: parent.path.clone(),
            query: restored_query.clone(),
            ancestors: self.levels.ancestors(),
        };
        bg::fetch(
            cx,
            move || sources::source_rows(block_id, args),
            move |this, level, cx| {
                if !this.levels.holds(token) {
                    return;
                }
                if let Some(error) = level.error {
                    // An empty level and a broken command look identical
                    // once inside one, so neither is entered.
                    this.banner.show(
                        format!("{title}: {error}"),
                        Tone::Error,
                        actions::BANNER_SECONDS,
                        cx,
                    );
                    return;
                }
                if level.truncated {
                    this.banner.show(
                        format!("{title}: showing the first {}", level.rows.len()),
                        Tone::Info,
                        actions::BANNER_SECONDS,
                        cx,
                    );
                }
                let frame =
                    levels::frame(level, title, &parent, restored_query, restored_selection);
                this.levels.push(frame);
                this.enter_level(cx);
            },
        );
    }

    /// A level was pushed: the query that got here means nothing now, and
    /// the launcher's other modes have nothing to say inside one.
    fn enter_level(&mut self, cx: &mut Context<Self>) {
        self.close_menu();
        self.ai.cancel();
        self.translation = None;
        self.set_query("", cx);
    }

    /// Escape: back one level, with the query and selection it opened from,
    /// so descending and coming back is free rather than a re-search.
    fn pop_level(&mut self, cx: &mut Context<Self>) {
        let Some(left) = self.levels.pop() else {
            return;
        };
        if let Some(id) = left.restored_selection {
            self.pending_restore = Some((left.restored_query.clone(), id));
        }
        self.set_query(&left.restored_query, cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.handle_key(&event.keystroke, cx);
    }

    fn handle_key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Self>) {
        let ctrl = ks.modifiers.control;
        let shift = ks.modifiers.shift;
        // Alt+digit: a running app.
        if ks.modifiers.alt
            && !ctrl
            && !shift
            && let Some(digit) = ks.key.chars().next().and_then(|c| c.to_digit(10))
            && (1..=9).contains(&digit)
            && self.running.activate_key(digit as u8, cx)
        {
            cx.stop_propagation();
            return;
        }
        // Alt+<letter> on the home screen is a launchpad mnemonic.
        if ks.modifiers.alt && !ctrl && self.on_home(cx) {
            let mut chars = ks.key.chars();
            if let (Some(ch), None) = (chars.next(), chars.next())
                && self
                    .launchpad
                    .update(cx, |launchpad, cx| launchpad.mnemonic(ch, cx))
            {
                cx.stop_propagation();
                return;
            }
        }
        // The confirm bar owns every key while it asks.
        if self.confirm.is_some() {
            match ks.key.as_str() {
                "y" | "enter" => self.settle_confirm(true, cx),
                "n" | "escape" => self.settle_confirm(false, cx),
                _ => {}
            }
            cx.stop_propagation();
            return;
        }
        // So does the action menu: movement, Enter and Escape.
        if self.menu.is_some() {
            let vim = ctrl && !shift && (ks.key == "j" || ks.key == "k");
            let consumed = match ks.key.as_str() {
                "down" => self.move_menu(1, cx),
                "up" => self.move_menu(-1, cx),
                "j" if vim => self.move_menu(1, cx),
                "k" if vim => self.move_menu(-1, cx),
                "enter" => {
                    let at = self.menu.as_ref().map_or(0, |m| m.focused);
                    self.activate_menu_item(at, cx);
                    true
                }
                "escape" => {
                    self.close_menu();
                    cx.notify();
                    true
                }
                _ => false,
            };
            if consumed {
                cx.stop_propagation();
                return;
            }
        }
        let menu = self.mode.is_menu();
        let handled = match ks.key.as_str() {
            "escape" => self.escape(cx),
            "enter" if ctrl => self.search_web(cx),
            "enter" if shift && !self.picked.is_empty() => {
                self.open_all_picked(cx);
                true
            }
            "enter" => self.enter(cx),
            "j" | "k" if ctrl && !shift && !menu => {
                self.open_menu(cx);
                true
            }
            "e" if ctrl && !menu => self.run_tool(actions::EDIT, cx),
            "t" if ctrl && !menu => self.run_tool(actions::TERMINAL, cx),
            "p" if ctrl && shift => {
                self.clear_picked(cx);
                true
            }
            "p" if ctrl && !menu => self.toggle_pick(cx),
            "h" if ctrl && shift => self.hide_selected_app(cx),
            "down" => self.move_selection(1, cx),
            "up" => self.move_selection(-1, cx),
            "tab" => self.move_selection(if shift { -1 } else { 1 }, cx),
            "n" if ctrl => self.move_selection(1, cx),
            "backspace" if ctrl => self.edit(cx, SearchInput::delete_word_back),
            "w" if ctrl => self.edit(cx, SearchInput::delete_word_back),
            "backspace" => self.edit(cx, SearchInput::backspace),
            "delete" => self.edit(cx, SearchInput::delete),
            "left" if ctrl => self.edit(cx, move |i, cx| i.word_left(shift, cx)),
            "right" if ctrl => self.edit(cx, move |i, cx| i.word_right(shift, cx)),
            "left" => self.edit(cx, move |i, cx| i.left(shift, cx)),
            "right" => self.edit(cx, move |i, cx| i.right(shift, cx)),
            "home" => self.edit(cx, move |i, cx| i.home(shift, cx)),
            "end" => self.edit(cx, move |i, cx| i.end(shift, cx)),
            "a" if ctrl => self.edit(cx, SearchInput::select_all),
            "u" if ctrl => self.edit(cx, SearchInput::clear),
            "v" if ctrl => self.paste(cx),
            "c" if ctrl => self.copy(cx),
            // Side actions have nothing to act on in a menu.
            "f" if ctrl && !menu => self.reveal_selected(cx),
            "d" if ctrl && !menu => self.delete(cx),
            "i" if ctrl && !menu => self.paste_selected_clip(cx),
            _ => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }

    fn edit(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(&mut SearchInput, &mut Context<SearchInput>),
    ) -> bool {
        self.input.update(cx, op);
        true
    }

    /// Escape leaves a level, then a mode, before it hides the launcher.
    fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        if self.levels.is_active() {
            self.pop_level(cx);
        } else if self.mode.is_prefixed() {
            self.set_query("", cx);
        } else {
            self.shell.hide();
        }
        true
    }

    /// Enter: what the mode says, else what the row says.
    fn enter(&mut self, cx: &mut Context<Self>) -> bool {
        match self.mode {
            Mode::Translate => self.translate(cx),
            Mode::Process => self
                .preview
                .update(cx, |preview, cx| preview.measure_cpu(cx)),
            _ => return self.open_selected(cx),
        }
        true
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        self.close_menu();
        if !self.rows.is_empty() {
            let len = self.rows.len() as isize;
            self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
            self.scroll
                .scroll_to_item(self.selected, ScrollStrategy::Nearest);
            self.sync_preview(cx);
            cx.notify();
        }
        true
    }

    /// The preview follows the selection; the compact layout has none, and
    /// the menus hide the column. An empty clipboard history shows its tips.
    fn sync_preview(&mut self, cx: &mut Context<Self>) {
        let split = theme::get().split();
        let row = self.rows.get(self.selected).cloned();
        let mode = self.mode;
        let ancestors = self.levels.ancestors();
        self.preview.update(cx, |preview, cx| match row {
            Some(row) if split && !mode.is_menu() => preview.show(&row, ancestors, cx),
            None if split && mode.is_clipboard() => preview.show_help(mode, cx),
            _ => preview.clear(cx),
        });
    }

    fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    /// Enter: what the row says. The backend opens paths and hides the
    /// launcher through the shell; a URL goes to the browser and the history;
    /// an answer or a clip goes to the clipboard.
    fn open_selected(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        match row.open {
            Open::Path { .. } if actions::is_source_row(&row.id) => {
                self.perform_source_row(row, cx)
            }
            Open::Path { usage } => {
                match launch::open_path(&self.shell, row.path, Some(&row.kind), Some(&row.id)) {
                    Ok(()) => {
                        engine::record_usage(app_state(), &row.id, usage);
                    }
                    Err(err) => eprintln!("open {}: {err}", row.title),
                }
            }
            Open::Url(url) => {
                self.open_url(&url);
                cx.background_executor()
                    .spawn(async move {
                        weburl::record_url_hit(&url);
                    })
                    .detach();
            }
            Open::Calc { raw, expr } => {
                // History keeps the working; the paste is the number.
                let label = format!("{expr} = {}", row.title);
                let shell = self.shell.clone();
                bg::fetch(
                    cx,
                    move || clipboard::copy_to_clipboard_labeled(raw, label),
                    move |this, outcome, cx| match outcome {
                        Ok(()) => shell.hide(),
                        Err(err) => this.banner.show(err, Tone::Error, banner::MEDIUM, cx),
                    },
                );
            }
            Open::Prefix(prefix) => self.set_query(&prefix, cx),
            Open::Command(id) => self.banner.show(
                format!("{COMMANDS_PENDING}: {id}"),
                Tone::Info,
                banner::LONG,
                cx,
            ),
            Open::Clip(_) | Open::ClipImage(_) => self.copy_selected_clip(cx),
            Open::Process(_) => self
                .preview
                .update(cx, |preview, cx| preview.measure_cpu(cx)),
            Open::WebSuggestion(text) => {
                self.open_url(&format!("{WEB_SEARCH_URL}{}", url_encode(&text)))
            }
        }
        true
    }

    pub(crate) fn open_url(&self, url: &str) {
        if let Err(err) = launch::open_path(&self.shell, url.to_string(), Some("browser"), None) {
            eprintln!("open {url}: {err}");
        }
    }

    /// Ctrl+Enter: the query as a web search.
    fn search_web(&mut self, cx: &mut Context<Self>) -> bool {
        let query = self.input.read(cx).committed();
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }
        self.open_url(&format!("{WEB_SEARCH_URL}{}", url_encode(trimmed)));
        true
    }

    /// Ctrl+F: through the declared file manager where the action applies,
    /// else the platform's own reveal. A block row with no path reveals the
    /// file that declared it.
    fn reveal_selected(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        if !matches!(row.open, Open::Path { .. }) {
            return true;
        }
        if row.path.is_empty() && actions::is_source_row(&row.id) {
            let args = actions::row_args(&row, &self.levels.ancestors());
            let shell = self.shell.clone();
            bg::fetch(
                cx,
                move || {
                    let file = sources::source_block(args)?.file?;
                    Some(launch::reveal_path(&file))
                },
                move |_, outcome, _| {
                    if let Some(Ok(())) = outcome {
                        shell.hide();
                    }
                },
            );
            return true;
        }
        if actions::applies(actions::REVEAL, &row.kind) {
            return self.run_tool(actions::REVEAL, cx);
        }
        match launch::reveal_path(&row.path) {
            Ok(()) => self.shell.hide(),
            Err(err) => eprintln!("reveal: {err}"),
        }
        true
    }

    /// Ctrl+C: the field's selection when there is one, else the process's
    /// PID or the row's path.
    fn copy(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(text) = self.input.read(cx).selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            return true;
        }
        let Some(row) = self.selected_row() else {
            return true;
        };
        if row.is_hint() {
            return true;
        }
        let (text, done) = match &row.open {
            Open::Process(p) => (p.pid.to_string(), format!("Copied PID {}", p.pid)),
            _ if row.path.is_empty() => return true,
            _ => (row.path.clone(), "Copied to clipboard".to_string()),
        };
        bg::fetch(
            cx,
            move || clipboard::copy_to_clipboard(&text),
            move |this, outcome, cx| match outcome {
                Ok(()) => this.banner.show(done, Tone::Success, banner::SHORT, cx),
                Err(_) => this
                    .banner
                    .show("Copy failed".into(), Tone::Error, banner::MEDIUM, cx),
            },
        );
        true
    }

    fn paste(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            // One line: the field is single-line, and a pasted path or query
            // never wants its newlines.
            let text = text.replace(['\r', '\n'], " ");
            self.input.update(cx, |input, cx| input.insert(&text, cx));
        }
        true
    }

    /// Ctrl+D: kill the process, forget the clip, or trash the file.
    fn delete(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        match row.open {
            Open::Process(p) => {
                let name = p.name.clone();
                bg::fetch(
                    cx,
                    move || process::kill_process(p.pid),
                    move |this, outcome, cx| {
                        match outcome {
                            Ok(_) => this.banner.show(
                                format!("Killed {name} ({})", p.pid),
                                Tone::Success,
                                banner::MEDIUM,
                                cx,
                            ),
                            Err(err) => this.banner.show(
                                format!("Kill failed: {err}"),
                                Tone::Error,
                                banner::LONG,
                                cx,
                            ),
                        }
                        // A fresh walk, so the killed row drops off.
                        this.refresh_processes = true;
                        this.search(cx);
                    },
                );
            }
            Open::Clip(clip) => bg::fetch(
                cx,
                move || clipboard::delete_clipboard_entry(clip.timestamp, &clip.text),
                |this, removed, cx| {
                    if removed {
                        this.clip_removed(cx);
                    }
                },
            ),
            Open::ClipImage(image) => bg::fetch(
                cx,
                move || clipboard::delete_clipboard_image(&image.hash),
                |this, removed, cx| {
                    if removed {
                        this.clip_removed(cx);
                    }
                },
            ),
            Open::Path { .. } if TRASH_PIN_IDS.contains(&row.id.as_str()) => {
                self.empty_trash(cx);
            }
            Open::Path { .. } if row.kind == "file" || row.kind == "folder" => {
                // The picks when there are any, as the webview trashes them.
                let paths = if self.picked.is_empty() {
                    vec![row.path]
                } else {
                    self.picked.paths()
                };
                self.clear_picked(cx);
                self.trash(paths, cx);
            }
            _ => self.banner.show(
                "Select a file or folder to delete".into(),
                Tone::Info,
                banner::MEDIUM,
                cx,
            ),
        }
        true
    }

    /// A clip left its history: say so and show the list without it.
    fn clip_removed(&mut self, cx: &mut Context<Self>) {
        let text = match self.mode {
            Mode::ClipboardImage => CLIP_IMAGE_DELETED,
            _ => CLIP_DELETED,
        };
        self.banner.show(text.into(), Tone::Info, banner::CLIP, cx);
        self.search(cx);
    }

    fn trash(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        bg::fetch(
            cx,
            move || trash::trash_paths(paths),
            |this, outcome, cx| {
                let (text, tone, seconds) = if outcome.failed.is_empty() {
                    (
                        format!("Moved {} to {TRASH_LABEL}", outcome.trashed),
                        Tone::Success,
                        banner::MEDIUM,
                    )
                } else if outcome.trashed == 0 {
                    let first = &outcome.failed[0];
                    let name = first.path.rsplit(['/', '\\']).next().unwrap_or(&first.path);
                    (
                        format!("Failed to trash {name}: {}", first.reason),
                        Tone::Error,
                        banner::READ,
                    )
                } else {
                    (
                        format!("Moved {}, {} failed", outcome.trashed, outcome.failed.len()),
                        Tone::Error,
                        banner::READ,
                    )
                };
                this.banner.show(text, tone, seconds, cx);
                app_state().request_index_refresh();
            },
        );
    }

    /// Enter on a clip: back onto the clipboard, in the form it was copied.
    fn copy_selected_clip(&mut self, cx: &mut Context<Self>) {
        let Some((work, done)) = self.selected_row().and_then(|row| clip_write(&row.open)) else {
            return;
        };
        bg::fetch(cx, work, move |this, outcome, cx| match outcome {
            Ok(()) => this
                .banner
                .show(done.into(), Tone::Success, banner::SHORT, cx),
            Err(err) => this.banner.show(err, Tone::Error, banner::MEDIUM, cx),
        });
    }

    /// Ctrl+I: the same copy, then out of the way and into the app
    /// underneath. The blocker is asked first, since a banner behind a hidden
    /// window explains nothing; the copy happens either way.
    fn paste_selected_clip(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((work, _)) = self.selected_row().and_then(|row| clip_write(&row.open)) else {
            return true;
        };
        let shell: Arc<dyn LauncherWindow> = Arc::new(self.shell.clone());
        bg::fetch(
            cx,
            move || {
                work()?;
                Ok(paste::clipboard_paste_blocker())
            },
            move |this, outcome: Result<Option<String>, String>, cx| match outcome {
                Err(err) => this.banner.show(err, Tone::Error, banner::MEDIUM, cx),
                Ok(Some(blocker)) => {
                    this.banner
                        .show(blocker, Tone::Warning, banner::PASTE_BLOCKED, cx)
                }
                Ok(None) => paste::paste_into_focused_app(shell),
            },
        );
        true
    }

    /// `t"` Enter: the three translations, each landing as it answers.
    fn translate(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).committed();
        let (_, term) = Mode::of(&query);
        let text = term.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.translation = Some(Translation {
            text: text.clone(),
            sections: vec![None; LANGUAGES.len()],
        });
        for (i, (code, _)) in LANGUAGES.iter().enumerate() {
            let text = text.clone();
            let wanted = text.clone();
            bg::fetch(
                cx,
                move || translate::translate(&text, code),
                move |this, result, _| {
                    let Some(translation) = this.translation.as_mut() else {
                        return;
                    };
                    if translation.text != wanted {
                        return;
                    }
                    translation.sections[i] = Some(match result.error {
                        Some(error) => Err(error),
                        None => Ok(result.translated),
                    });
                },
            );
        }
        cx.notify();
    }

    // --- Picked ----------------------------------------------------------------

    /// Ctrl+P: in or out of the picks. Files and folders only.
    fn toggle_pick(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        if !picked::pickable(&row) {
            self.banner.show(
                "Only files or folders can be picked".into(),
                Tone::Info,
                banner::MEDIUM,
                cx,
            );
            return true;
        }
        self.picked.toggle(&row);
        self.picks_changed(cx);
        true
    }

    pub(crate) fn remove_picked(&mut self, at: usize, cx: &mut Context<Self>) {
        self.picked.remove(at);
        self.picks_changed(cx);
    }

    pub(crate) fn clear_picked(&mut self, cx: &mut Context<Self>) {
        if self.picked.is_empty() {
            return;
        }
        self.picked.clear();
        self.picks_changed(cx);
    }

    /// The picks go to the clipboard as files, so a paste in a file manager
    /// lands them, and the banner counts them.
    fn picks_changed(&mut self, cx: &mut Context<Self>) {
        let count = self.picked.len();
        if count == 0 {
            self.sync_preview(cx);
            cx.notify();
            return;
        }
        let paths = self.picked.paths();
        bg::fetch(
            cx,
            move || files::copy_files_to_clipboard(&paths),
            move |this, outcome, cx| {
                let (text, tone, seconds) = match outcome {
                    Ok(()) => (
                        format!("Picked {count} item(s)"),
                        Tone::Success,
                        banner::SHORT,
                    ),
                    Err(_) => ("Pick failed".to_string(), Tone::Error, banner::MEDIUM),
                };
                this.banner.show(text, tone, seconds, cx);
            },
        );
        cx.notify();
    }

    /// Shift+Enter: every pick, opened in turn, then the picks cleared.
    pub(crate) fn open_all_picked(&mut self, cx: &mut Context<Self>) {
        for row in self.picked.items().to_vec() {
            let usage = match row.open {
                Open::Path { usage } => usage,
                _ => continue,
            };
            match launch::open_path(
                &self.shell,
                row.path.clone(),
                Some(&row.kind),
                Some(&row.id),
            ) {
                Ok(()) => {
                    engine::record_usage(app_state(), &row.id, usage);
                }
                Err(err) => eprintln!("open {}: {err}", row.title),
            }
        }
        self.clear_picked(cx);
    }

    // --- Confirm ---------------------------------------------------------------

    fn ask(&mut self, confirm: Confirm, cx: &mut Context<Self>) {
        self.close_menu();
        self.confirm = Some(confirm);
        cx.notify();
    }

    pub(crate) fn settle_confirm(&mut self, yes: bool, cx: &mut Context<Self>) {
        let Some(mut confirm) = self.confirm.take() else {
            return;
        };
        if yes && let Some(on_yes) = confirm.on_yes.take() {
            on_yes(self, cx);
        }
        cx.notify();
    }

    /// Ctrl+D on the trash pin: how many it holds, then the question.
    fn empty_trash(&mut self, cx: &mut Context<Self>) {
        bg::fetch(
            cx,
            trash::count_trash_items,
            |this, count, cx| match count {
                Err(err) => this.banner.show(
                    format!("Empty {TRASH_LABEL} unavailable: {err}"),
                    Tone::Error,
                    banner::READ,
                    cx,
                ),
                Ok(0) => this.banner.show(
                    format!("{TRASH_LABEL} is already empty"),
                    Tone::Info,
                    banner::MEDIUM,
                    cx,
                ),
                Ok(count) => {
                    let items = if count == 1 { "item" } else { "items" };
                    let on_yes: OnYes = Box::new(|_, cx| {
                        bg::fetch(cx, trash::empty_trash, |this, purged, cx| {
                            match purged {
                                Ok(n) => this.banner.show(
                                    format!("Emptied {TRASH_LABEL} ({n})"),
                                    Tone::Success,
                                    banner::MEDIUM,
                                    cx,
                                ),
                                Err(err) => this.banner.show(
                                    format!("Empty {TRASH_LABEL} failed: {err}"),
                                    Tone::Error,
                                    banner::READ,
                                    cx,
                                ),
                            }
                            app_state().request_index_refresh();
                        });
                    });
                    this.ask(
                        Confirm {
                            title: format!("Empty {TRASH_LABEL}?"),
                            detail: format!("{count} {items} - deleted permanently"),
                            glyph: Some(glyphs::TRASH),
                            on_yes: Some(on_yes),
                        },
                        cx,
                    );
                }
            },
        );
    }

    /// Ctrl+Shift+H: the selected app joins `app_exclude_names`, after a
    /// question, and the config reloads so it leaves the index.
    fn hide_selected_app(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        if row.kind != "app"
            || row.path.is_empty()
            || row.is_hint()
            || !matches!(row.open, Open::Path { .. })
        {
            self.banner.show(
                "Select an app to hide".into(),
                Tone::Warning,
                banner::MEDIUM,
                cx,
            );
            return true;
        }
        let title = row.title.trim().to_string();
        let on_yes: OnYes = Box::new(move |this, cx| {
            let name = title.clone();
            bg::fetch(
                cx,
                move || hide_app(&name),
                move |this, outcome, cx| match outcome {
                    Ok(true) => this.banner.show(
                        format!("Hidden {title}"),
                        Tone::Success,
                        banner::MEDIUM,
                        cx,
                    ),
                    Ok(false) => this.banner.show(
                        format!("{title} is already hidden"),
                        Tone::Info,
                        banner::MEDIUM,
                        cx,
                    ),
                    Err(err) => this.banner.show(
                        format!("Hide app failed: {err}"),
                        Tone::Error,
                        banner::LONG,
                        cx,
                    ),
                },
            );
            let _ = this;
        });
        self.ask(
            Confirm {
                title: "Hide this app from Look?".into(),
                detail: format!("{} will be added to {APP_EXCLUDE_KEY}", row.title),
                glyph: None,
                on_yes: Some(on_yes),
            },
            cx,
        );
        true
    }

    // --- Row actions and the menu --------------------------------------------

    fn close_menu(&mut self) {
        self.menu_token += 1;
        self.menu = None;
    }

    fn move_menu(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if let Some(menu) = self.menu.as_mut() {
            menu.move_by(delta);
            cx.notify();
        }
        true
    }

    /// Ctrl+K or Ctrl+J: the row's verbs, resolved off the UI thread. A row
    /// with nothing to offer says so rather than showing an empty box.
    fn open_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.is_some() {
            return;
        }
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        if !actions::actionable(&row) {
            self.banner.show(
                actions::EMPTY.into(),
                Tone::Info,
                actions::MENU_BANNER_SECONDS,
                cx,
            );
            return;
        }
        self.menu_token += 1;
        let token = self.menu_token;
        let panel = if theme::get().split() {
            Vec::new()
        } else {
            actions::panel_items(
                !self.picked.is_empty(),
                row.kind == crate::rows::KIND_CLIPBOARD,
            )
        };
        let ancestors = self.levels.ancestors();
        bg::fetch(
            cx,
            move || actions::descriptors(&row, ancestors, panel),
            move |this, items, cx| {
                if this.menu_token != token {
                    return;
                }
                if items.is_empty() {
                    this.banner.show(
                        actions::EMPTY.into(),
                        Tone::Info,
                        actions::MENU_BANNER_SECONDS,
                        cx,
                    );
                    return;
                }
                this.menu = Some(Menu::new(items));
            },
        );
    }

    /// The one entry point for a menu row, by key or by click, so neither
    /// can skip the question a target asks.
    pub(crate) fn activate_menu_item(&mut self, at: usize, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.as_mut() else {
            return;
        };
        let Some(item) = menu.items.get(at).cloned() else {
            return;
        };
        if let Some(pending) = menu.pending.take() {
            self.close_menu();
            if item.id == ActionId::Yes {
                self.run_action(pending, cx);
            }
            cx.notify();
            return;
        }
        if item.confirm.is_some() {
            menu.ask(item.confirm, item.id);
            cx.notify();
            return;
        }
        // The target's title comes from the menu it was picked in, so the
        // menu goes after the action reads it.
        self.menu_token += 1;
        self.run_action(item.id, cx);
        self.close_menu();
        cx.notify();
    }

    fn run_action(&mut self, id: ActionId, cx: &mut Context<Self>) {
        match id {
            ActionId::Open => {
                self.open_selected(cx);
            }
            ActionId::Tool(action) => {
                self.run_tool(action, cx);
            }
            ActionId::CopyPath => {
                self.copy(cx);
            }
            ActionId::OpenAllPicked => self.open_all_picked(cx),
            ActionId::ClearPicked => self.clear_picked(cx),
            ActionId::DeleteClipboard => {
                self.delete(cx);
            }
            ActionId::Target(block_id) => self.run_target(block_id, cx),
            ActionId::Yes | ActionId::No => {}
        }
    }

    /// Perform one tool action on the selected row. The backend hides the
    /// launcher before it spawns and brings it back only when nothing
    /// started, so a failure has a window to report itself in.
    fn run_tool(&mut self, action: &'static str, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        if !actions::actionable(&row) || row.path.is_empty() || !actions::applies(action, &row.kind)
        {
            return true;
        }
        let args = actions::row_args(&row, &self.levels.ancestors());
        let is_dir = actions::is_dir(&row);
        let shell = self.shell.clone();
        bg::fetch(
            cx,
            move || tools::perform_tool_action(&shell, action, args, is_dir),
            |this, outcome, cx| {
                // An action that could not run explains itself here rather
                // than being greyed out with a tool name nobody could find.
                if let Some(reason) = outcome.and_then(|r| r.reason) {
                    this.banner
                        .show(reason, Tone::Info, actions::BANNER_SECONDS, cx);
                }
            },
        );
        true
    }

    /// Enter on a block row: what the block's `open` says, with its question
    /// asked first in the menu. Core says whether the row's own path opens.
    fn perform_source_row(&mut self, row: Row, cx: &mut Context<Self>) {
        let Some(block_id) = actions::block_id_of(&row.id).map(str::to_string) else {
            self.banner.show(
                "Couldn't tell which block this row belongs to".into(),
                Tone::Error,
                actions::BANNER_SECONDS,
                cx,
            );
            return;
        };
        let ancestors = self.levels.ancestors();
        let args = actions::row_args(&row, &ancestors);
        let title = row.title.clone();
        let shell = self.shell.clone();
        bg::fetch(
            cx,
            move || {
                let declared = sources::source_block(actions::row_args(&row, &ancestors));
                if let Some(question) = declared.and_then(|d| d.confirm) {
                    return Err(question);
                }
                let outcome = sources::perform_block(block_id, args, false);
                engine::record_usage(app_state(), &row.id, USAGE_EXECUTE);
                if outcome.opens_path {
                    let _ =
                        launch::open_path(&shell, row.path.clone(), Some(&row.kind), Some(&row.id));
                    return Ok(None);
                }
                match outcome.errors.into_iter().next() {
                    Some(failure) => Ok(Some(failure)),
                    None => {
                        shell.hide();
                        Ok(None)
                    }
                }
            },
            move |this, outcome, cx| match outcome {
                // Asked in the menu rather than a modal, as a `then` target is.
                Err(question) => {
                    let mut menu = Menu::new(Vec::new());
                    menu.ask(Some(question), ActionId::Open);
                    this.menu_token += 1;
                    this.menu = Some(menu);
                }
                Ok(Some(failure)) => this.banner.show(
                    format!("{title}: {failure}"),
                    Tone::Error,
                    actions::BANNER_SECONDS,
                    cx,
                ),
                Ok(None) => {}
            },
        );
    }

    /// A `then` target against the selected row: performed, or a level to
    /// descend into when it produces rows.
    fn run_target(&mut self, block_id: String, cx: &mut Context<Self>) {
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        let title = self
            .menu
            .as_ref()
            .and_then(|m| {
                m.items.iter().find_map(|item| match &item.id {
                    ActionId::Target(id) if *id == block_id => {
                        Some(item.title.trim_end_matches('\u{2026}').to_string())
                    }
                    _ => None,
                })
            })
            .unwrap_or_else(|| block_id.clone());
        let args = actions::row_args(&row, &self.levels.ancestors());
        let shell = self.shell.clone();
        let id = block_id.clone();
        bg::fetch(
            cx,
            move || {
                let outcome = sources::perform_block(id, args, true);
                if let Some(failure) = outcome.errors.into_iter().next() {
                    return Err(failure);
                }
                if !outcome.produces_rows {
                    shell.hide();
                }
                Ok(outcome.produces_rows)
            },
            move |this, outcome, cx| match outcome {
                Err(failure) => this.banner.show(
                    format!("{title}: {failure}"),
                    Tone::Error,
                    actions::BANNER_SECONDS,
                    cx,
                ),
                // Not a failure and nothing performed: the target lists.
                Ok(true) => this.descend(block_id, title, row, cx),
                Ok(false) => {}
            },
        );
    }

    fn top_bar(&mut self, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let bar_h = theme::TOP_ROW_HEIGHT + 2.0 * theme::INPUT_PADDING_Y;
        let crumbs = self.levels.breadcrumb();
        let placeholder = if crumbs.is_empty() { PLACEHOLDER } else { "" };
        // The strip steps aside for the translate panel and the compact layout.
        let strip = (th.split() && self.mode != Mode::Translate)
            .then(|| self.running.render(&self.icons, th, cx))
            .flatten();
        let bar = card(div(), th)
            .absolute()
            .left_0()
            .right_0()
            .h(px(bar_h))
            .px(px(theme::INPUT_PADDING_X))
            .flex()
            .items_center()
            .gap(px(theme::SEARCH_GAP))
            .rounded(px(th.bar_radius()))
            .text_size(px(th.font_size + 1.0))
            .child(
                svg()
                    .path(glyphs::SEARCH)
                    .size(px(theme::SEARCH_ICON))
                    .text_color(th.accent),
            )
            .when(!crumbs.is_empty(), |el| {
                el.child(
                    muted_text(crumbs.join(BREADCRUMB_SEPARATOR), th)
                        .flex_shrink_0()
                        .text_size(px(th.font_size)),
                )
            })
            .child(search_field(self.input.clone(), placeholder))
            .children((!th.split()).then(|| self.picked.badge(th)).flatten())
            .children(strip)
            .child(hint_chip("Esc", th));

        // The bar rises in place, so its slot is fixed and the body below
        // does not move with it.
        let spawn = Duration::from_millis(motion::SPAWN_MS);
        let radius = th.bar_radius();
        div()
            .relative()
            .h(px(bar_h))
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(theme::CONTENT_PADDING))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .child(
                bar.with_animation("top-bar", Animation::new(spawn), |bar, t| {
                    let t = motion::curve(t);
                    bar.opacity(t.min(1.0))
                        .top(px(motion::rise(0.0, motion::SPAWN_RISE, t)))
                }),
            )
    }

    /// The empty query shows the launchpad: the tiles the entity draws, in a
    /// grid whose seams are the inner gap, each marked for the blur region.
    fn bento(&mut self, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (grid_h, tiles) = self
            .launchpad
            .update(cx, |launchpad, cx| launchpad.tiles_in(th, cx));
        let radius = th.tile_radius();
        div()
            .relative()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(th.inner_gap))
            .h(px(grid_h))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .children(tiles)
    }

    /// Whether the empty query is showing, launchpad or not.
    fn on_home(&self, cx: &gpui::App) -> bool {
        self.input.read(cx).text().is_empty()
    }

    fn row(&self, i: usize, row: &Row, th: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let selected = i == self.selected;
        let picture = match &row.icon {
            Icon::Resolve { glyph } => {
                let image = self.icons.update(cx, |store, cx| {
                    store.get(
                        IconRequest {
                            kind: row.icon_kind().to_string(),
                            path: row.path.clone(),
                            id: Some(row.id.clone()),
                        },
                        cx,
                    )
                });
                match image {
                    Some(image) => img(image).size(px(ROW_ICON)).into_any_element(),
                    None => glyph_icon(glyph, th).into_any_element(),
                }
            }
            Icon::Glyph(glyph) => glyph_icon(glyph, th).into_any_element(),
            Icon::File(path) => img(path.clone())
                .size(px(ROW_ICON))
                .rounded(px(th.chip_radius() / 1.5))
                .object_fit(gpui::ObjectFit::Cover)
                .into_any_element(),
            Icon::Text(text) => div()
                .text_size(px(ROW_ICON * 0.8))
                .child(SharedString::from(text.clone()))
                .into_any_element(),
        };
        div()
            .id(("row", i))
            .w_full()
            .h(px(theme::ROW_HEIGHT))
            .px(px(theme::ROW_PADDING_X))
            .flex()
            .items_center()
            .gap(px(theme::SEARCH_GAP))
            .rounded(px(th.control_radius()))
            .when(selected, |el| el.bg(th.selection_fill))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = i;
                this.sync_preview(cx);
                this.enter(cx);
            }))
            .child(
                div()
                    .size(px(theme::ICON_CHIP))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(picture),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(row.title.clone()),
                    )
                    .when(!row.context.is_empty(), |col| {
                        col.child(muted_text(row.context.clone(), th).truncate())
                    }),
            )
            .when(!row.kind_label.is_empty(), |el| {
                el.child(muted_text(row.kind_label.clone(), th))
            })
            .when(self.picked.contains(row), |el| el.child(picked::check(th)))
            .when(selected, |el| el.child(hint_chip("Enter", th)))
            .into_any_element()
    }

    /// What the list card shows with no rows: a history's info half, the
    /// recent hint, or two words.
    fn empty_state(&self, th: &Theme) -> Div {
        let rich = |glyph: &'static str, title: &str, body: &str, help: &str| {
            div()
                .flex_1()
                .min_h(px(EMPTY_STATE_H))
                .px(px(EMPTY_STATE_PADDING_X))
                .py(px(EMPTY_STATE_PADDING_Y))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(EMPTY_STATE_GAP))
                .text_center()
                .child(
                    svg()
                        .path(glyph)
                        .size(px(EMPTY_STATE_ICON))
                        .text_color(th.accent)
                        .opacity(0.85),
                )
                .child(
                    div()
                        .text_size(px(th.font_size + 1.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from(title.to_string())),
                )
                .child(
                    div()
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_secondary)
                        .child(SharedString::from(body.to_string())),
                )
                .child(
                    div()
                        .mt(px(EMPTY_STATE_GAP))
                        .max_w(px(EMPTY_HELP_MAX_W))
                        .text_size(px(th.font_size - 2.0))
                        .text_color(th.text_muted)
                        .child(SharedString::from(help.to_string())),
                )
        };
        match self.mode {
            Mode::Clipboard => rich(
                glyphs::CLIPBOARD,
                "Clipboard History",
                "No clipboard items yet",
                "Copy any text, then search with c\"word to find it here.",
            ),
            Mode::ClipboardImage => rich(
                glyphs::IMAGE,
                "Copied Images",
                "No images copied yet",
                "Copy a picture or take a screenshot, then find it here with ci\"name.",
            ),
            Mode::Recent => rich(
                glyphs::HISTORY,
                "Recent files & folders",
                "Nothing recent yet",
                "Open files/folders through Look, or download/create some - newest activity shows here. Type rc\"word to filter.",
            ),
            // While the answer card streams, a stark "No results" beside it
            // reads as broken.
            _ if self.ai.is_active() => div(),
            _ => div()
                .h(px(EMPTY_STATE_H))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_muted)
                .child(NO_RESULTS),
        }
    }

    fn results(&mut self, composing: bool, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows.clone();
        let row_theme = th.clone();
        let launcher = cx.entity();
        let list = uniform_list("results", rows.len(), move |range, _window, cx| {
            launcher.update(cx, |this, cx| {
                range
                    .map(|i| this.row(i, &rows[i], &row_theme, cx))
                    .collect()
            })
        })
        .track_scroll(&self.scroll)
        .flex_1()
        .min_h_0();

        let hint = if composing {
            HINT_COMPOSING
        } else if self.levels.is_active() {
            HINT_LEVEL
        } else if self.rows.is_empty() && self.mode == Mode::Search {
            HINT_EMPTY
        } else {
            self.mode.hint()
        };

        // One floating card: rows, then the hint as the card's own footer.
        let radius = th.tile_radius();
        let results_card = card(div(), th)
            .px(px(theme::ROW_INSET))
            .pt(px(theme::ROW_INSET))
            .pb(px(theme::HINT_INSET_BOTTOM))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(theme::ROW_SPACING))
            .rounded(px(radius))
            .children(
                self.ai
                    .render(th, cx)
                    .map(|card| card.mx(px(theme::ROW_PADDING_X))),
            )
            .map(|card| {
                if self.rows.is_empty() {
                    card.child(self.empty_state(th)).child(div().flex_1())
                } else {
                    card.child(list)
                }
            })
            .child(
                muted_text(hint, th)
                    .px(px(theme::ROW_PADDING_X))
                    .pt(px(theme::HINT_INSET))
                    .pb(px(theme::HINT_INSET_BOTTOM)),
            );
        // Split: the preview floats beside the list as its own card, except
        // for the menus, whose rows have nothing to describe.
        let preview_card = (th.split() && !self.mode.is_menu()).then(|| {
            let panel_theme = th.clone();
            // The picks take the column while there are any.
            let body = if self.picked.is_empty() {
                self.preview
                    .update(cx, |preview, cx| preview.render_in(&panel_theme, cx))
                    .into_any_element()
            } else {
                self.picked.panel(&self.icons, th, cx).into_any_element()
            };
            card(div(), th)
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .rounded(px(radius))
                .child(body)
        });
        let menu = self
            .menu
            .as_ref()
            .map(|menu| menu.render(th.split(), th, cx));
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(th.inner_gap))
            .mb(px(theme::CONTENT_PADDING))
            .flex()
            .flex_row()
            .gap(px(th.inner_gap))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .child(results_card)
            .children(preview_card)
            .children(menu)
    }

    /// `t"`: the whole content row, before and after Enter.
    fn translate_panel(&self, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let radius = th.tile_radius();
        let body: AnyElement = match &self.translation {
            None => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(theme::SEARCH_GAP))
                .child(
                    svg()
                        .path(glyphs::GLOBE)
                        .size(px(TRANSLATE_ICON))
                        .text_color(th.text_muted),
                )
                .child(muted_text(TRANSLATE_PLACEHOLDER, th))
                .into_any_element(),
            Some(translation) => {
                let source = div()
                    .py(px(TRANSLATE_SECTION_PADDING_Y))
                    .flex()
                    .items_baseline()
                    .gap(px(theme::SEARCH_GAP))
                    .child(
                        div()
                            .text_size(px(th.font_size + 2.0))
                            .font_weight(FontWeight::BOLD)
                            .child(translation.text.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(th.font_size - 3.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(th.accent)
                            .child(TRANSLATE_BADGE),
                    );
                let sections = LANGUAGES.iter().enumerate().map(|(i, (_, label))| {
                    let (text, colour, copy) = match &translation.sections[i] {
                        None => (TRANSLATING.to_string(), th.text_muted, None),
                        Some(Ok(text)) => (text.clone(), th.text, Some(text.clone())),
                        Some(Err(err)) => (
                            if err.is_empty() {
                                TRANSLATE_FAILED.to_string()
                            } else {
                                err.clone()
                            },
                            th.danger,
                            None,
                        ),
                    };
                    div()
                        .py(px(TRANSLATE_SECTION_PADDING_Y))
                        .border_b(px(1.0))
                        .border_color(th.border)
                        .flex()
                        .flex_col()
                        .gap(px(theme::CHIP_PADDING_X))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(th.font_size - 2.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(th.text_secondary)
                                        .child(*label),
                                )
                                .when_some(copy, |el, text| {
                                    el.child(
                                        div()
                                            .id(("translate-copy", i))
                                            .p(px(theme::CHIP_PADDING_X))
                                            .rounded(px(th.chip_radius()))
                                            .text_color(th.text_muted)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(th.control_fill).text_color(th.text))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let text = text.clone();
                                                bg::fetch(
                                                    cx,
                                                    move || clipboard::copy_to_clipboard(&text),
                                                    |this, outcome, cx| {
                                                        let (msg, tone) = match outcome {
                                                            Ok(()) => ("Copied", Tone::Success),
                                                            Err(_) => ("Copy failed", Tone::Error),
                                                        };
                                                        this.banner.show(
                                                            msg.into(),
                                                            tone,
                                                            banner::SHORT,
                                                            cx,
                                                        );
                                                    },
                                                );
                                                let _ = this;
                                            }))
                                            .child(
                                                svg()
                                                    .path(glyphs::COPY)
                                                    .size(px(theme::SEARCH_ICON))
                                                    .text_color(th.text_muted),
                                            ),
                                    )
                                }),
                        )
                        .child(div().text_color(colour).child(text))
                });
                let url = format!("{TRANSLATE_URL}{}", url_encode(&translation.text));
                let footer = div()
                    .id("translate-open")
                    .mt(px(TRANSLATE_PADDING_Y))
                    .py(px(TRANSLATE_SECTION_PADDING_Y))
                    .flex()
                    .items_center()
                    .gap(px(theme::SEARCH_GAP))
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_secondary)
                    .cursor_pointer()
                    .hover(|s| s.text_color(th.accent))
                    .on_click(cx.listener(move |this, _, _, _| this.open_url(&url)))
                    .child(
                        svg()
                            .path(glyphs::LINK)
                            .size(px(theme::SEARCH_ICON))
                            .text_color(th.accent),
                    )
                    .child(div().flex_1().child(OPEN_IN_BROWSER))
                    .child(
                        svg()
                            .path(glyphs::EXTERNAL_LINK)
                            .size(px(theme::SEARCH_ICON))
                            .text_color(th.text_muted),
                    );
                div()
                    .id("translate")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .child(source)
                    .children(sections)
                    .child(footer)
                    .into_any_element()
            }
        };
        div()
            .flex_1()
            .min_h_0()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(th.inner_gap))
            .mb(px(theme::CONTENT_PADDING))
            .flex()
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .child(
                card(div(), th)
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .px(px(theme::CONTENT_PADDING))
                    .pt(px(TRANSLATE_PADDING_Y))
                    .pb(px(theme::HINT_INSET_BOTTOM))
                    .rounded(px(radius))
                    .flex()
                    .flex_col()
                    .child(body)
                    .child(
                        muted_text(Mode::Translate.hint(), th)
                            .pt(px(theme::HINT_INSET))
                            .pb(px(theme::HINT_INSET_BOTTOM)),
                    ),
            )
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        blur::detach();
    }
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = theme::get();
        let input = self.input.read(cx);
        let home = input.text().is_empty() && !self.levels.is_active();
        let composing = input.is_composing();
        let focus_handle = input.focus_handle.clone();

        // The launchpad is a setting; off, the empty query is the bar alone.
        let below = match (home, th.launchpad, self.mode) {
            (true, true, _) => self.bento(&th, cx).into_any_element(),
            (true, false, _) => div().into_any_element(),
            (false, _, Mode::Translate) => self.translate_panel(&th, cx).into_any_element(),
            (false, _, _) => self.results(composing, &th, cx).into_any_element(),
        };
        let bar_radius = th.bar_radius();
        let banner = self.banner.render(&th, cx).map(|banner| {
            div()
                .on_children_prepainted(move |cards, _, _| mark_cards(&cards, bar_radius))
                .child(banner)
        });
        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .children(banner)
            .child(self.top_bar(&th, cx))
            .child(below);

        // gpui 0.2.2 has no element scale, and an inset in its place relayouts
        // every card each frame, so the arrive is the fade alone; the bar and
        // the tiles carry the motion.
        let arrive = Duration::from_millis(motion::ARRIVE_MS);

        div()
            .when(crate::probe_wanted(), |root| root.child(pace_probe()))
            .size_full()
            .font_family(th.font_family.clone())
            .text_size(px(th.font_size))
            .text_color(th.text)
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_children_prepainted(|_, _, _| commit_blur_region())
            .child(div().size_full().child(body).with_animation(
                "arrive",
                Animation::new(arrive),
                |root, t| root.opacity(motion::curve(t)),
            ))
            .children(self.confirm.as_ref().map(|confirm| confirm.render(&th, cx)))
    }
}

/// Add `name` to `app_exclude_names` and reload. `Ok(false)` when it was
/// there already.
fn hide_app(name: &str) -> Result<bool, String> {
    let current = config::get_config()
        .entries
        .into_iter()
        .find(|e| e.key == APP_EXCLUDE_KEY)
        .map(|e| e.value)
        .unwrap_or_default();
    let mut names = parse_config_list(&current);
    if names.iter().any(|n| n.trim().eq_ignore_ascii_case(name)) {
        return Ok(false);
    }
    names.push(name.to_string());
    config::set_config(vec![config::ConfigUpdate {
        key: APP_EXCLUDE_KEY.into(),
        value: render_config_list(&names),
    }])?;
    engine::reload_config(app_state());
    Ok(true)
}

/// The backend's CSV contract for config lists: `\,` is a literal comma and
/// `\\` a literal backslash.
fn parse_config_list(value: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if matches!(chars.peek(), Some(',') | Some('\\')) => {
                current.push(chars.next().unwrap_or_default());
            }
            ',' => {
                let entry = current.trim();
                if !entry.is_empty() {
                    entries.push(entry.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let entry = current.trim();
    if !entry.is_empty() {
        entries.push(entry.to_string());
    }
    entries
}

fn render_config_list(entries: &[String]) -> String {
    entries
        .iter()
        .map(|e| e.replace('\\', "\\\\").replace(',', "\\,"))
        .collect::<Vec<_>>()
        .join(",")
}

thread_local! {
    /// Card bounds gathered during one prepaint pass; the root hands them to
    /// the compositor once every card is placed.
    static BLUR_FRAME: RefCell<Vec<BlurRect>> = const { RefCell::new(Vec::new()) };
}

fn mark_cards(cards: &[Bounds<Pixels>], radius: f32) {
    BLUR_FRAME.with_borrow_mut(|frame| {
        for b in cards {
            frame.extend(BlurRect::rounded(
                f32::from(b.origin.x).round() as i32,
                f32::from(b.origin.y).round() as i32,
                f32::from(b.size.width).round() as u32,
                f32::from(b.size.height).round() as u32,
                radius,
            ));
        }
    });
}

fn commit_blur_region() {
    blur::set_region(BLUR_FRAME.take());
}

/// The frosted card face shared by the bar, the tiles and the results card.
fn card(el: Div, th: &Theme) -> Div {
    el.bg(th.card_face())
        .border(px(th.border_thickness))
        .border_color(th.border)
        .shadow(th.card_shadow())
}

fn muted_text(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_muted)
        .child(text.into())
}

fn hint_chip(label: &'static str, th: &Theme) -> Div {
    div()
        .px(px(theme::CHIP_PADDING_X))
        .py(px(theme::CHIP_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .bg(th.panel_fill)
        .text_size(px(theme::CAPTION_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_secondary)
        .child(label)
}

/// The glyph a row wears until its picture lands, in the accent like the
/// webview's kind glyphs.
fn glyph_icon(path: &'static str, th: &Theme) -> impl IntoElement {
    svg()
        .path(path)
        .size(px(theme::SEARCH_ICON + 2.0))
        .text_color(th.accent)
}

/// What putting a clip back on the clipboard runs, and what the banner says
/// once it has.
type ClipWrite = Box<dyn FnOnce() -> Result<(), String> + Send>;

fn clip_write(open: &Open) -> Option<(ClipWrite, &'static str)> {
    match open {
        Open::Clip(clip) => {
            let text = clip.payload.clone().unwrap_or_else(|| clip.text.clone());
            Some((
                Box::new(move || clipboard::copy_to_clipboard(&text)),
                "Copied to clipboard",
            ))
        }
        Open::ClipImage(image) => {
            let hash = image.hash.clone();
            Some((
                Box::new(move || clipboard::copy_clipboard_image(hash)),
                "Copied image",
            ))
        }
        _ => None,
    }
}

/// Percent-encoding for a query in a URL: letters, digits and the unreserved
/// marks pass, everything else is bytes.
fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A repeating animation on an invisible element, one timestamp per frame,
/// so tools/pace.sh can read steady-state pacing without the entrance in it.
fn pace_probe() -> impl IntoElement {
    div().absolute().size(px(1.0)).with_animation(
        "pace-probe",
        Animation::new(Duration::from_secs(1)).repeat(),
        |probe, t| {
            let micros = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros())
                .unwrap_or(0);
            eprintln!("probe {micros} t={t:.3}");
            probe.opacity(t)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_lists_round_trip_their_escapes() {
        let names = vec!["Zed".to_string(), "a,b".to_string(), "c\\d".to_string()];
        let rendered = render_config_list(&names);
        assert_eq!(rendered, "Zed,a\\,b,c\\\\d");
        assert_eq!(parse_config_list(&rendered), names);
        assert_eq!(parse_config_list(" , x ,"), vec!["x".to_string()]);
    }
}
