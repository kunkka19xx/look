//! The home launchpad: the bento the empty query shows, drawn from the shared
//! catalog (`core/qactions`, or the user's `~/.look/super-actions.toml`) the
//! way `superactions.js` and the macOS `EmptyStateLaunchpadView` draw it.
//! Live state comes from the backend adapters off the UI thread; a press
//! flips the tile at once and the adapter's answer reconciles it.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, Offset, Timelike};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, Div, EventEmitter, FontWeight, HighlightStyle,
    Image, Rgba, SharedString, Stateful, StyledText, Task, div, img, prelude::*, px, svg,
};
use linows_backend::look_engine::launchpad::{LayoutPayload, TileValue};
use linows_backend::look_lunar::LunarDate;
use linows_backend::look_qactions::{LaunchpadTile, TileRole, action_id};
use linows_backend::nowplaying::{self, NowPlayingSnapshot};
use linows_backend::qactions::{
    self, ActionIntent, ActionOutcome, ActionState, BATTERY_CHARGING_INFO_KEY,
    BATTERY_CHARGING_INFO_TEXT, InfoValue,
};
use linows_backend::weather::{self, WeatherSnapshot};
use linows_backend::{lunar, sysinfo, todo};

use crate::glyphs;
use crate::icons;
use crate::motion;
use crate::theme::{self, Theme};

/// One grid row, the macOS `Launchpad.rowHeight`.
pub const ROW_H: f32 = 76.0;

// Tile anatomy, from superactions.css.
const SLOT_PADDING: f32 = 16.0;
const TILE_GAP: f32 = 10.0;
const MEDIA_GAP: f32 = 14.0;
const ACTION_GAP: f32 = 8.0;
const TEXT_GAP: f32 = 2.0;
const INFO_TEXT_GAP: f32 = 3.0;
const HEAD_GAP: f32 = 12.0;
const CUSTOM_HEAD_GAP: f32 = 8.0;
const ICON_TOGGLE: f32 = 20.0;
const ICON_INFO: f32 = 22.0;
const ICON_ACTION: f32 = 24.0;
const ICON_WEATHER: f32 = 30.0;
const ICON_SLOT: f32 = 20.0;
const ICON_CUSTOM_HEAD: f32 = 18.0;
const ICON_RAIN: f32 = 12.0;
const SLOT_BADGE: f32 = 40.0;
const LABEL_SIZE: f32 = 14.0;
const LABEL_SMALL: f32 = 13.0;
const STATE_SIZE: f32 = 12.0;
const HEAD_TIME_SIZE: f32 = 22.0;
const HEAD_LUNAR_SIZE: f32 = 18.0;
const HEAD_LUNAR_LINE_SIZE: f32 = 11.0;
const BODY_TIME_SIZE: f32 = 30.0;
const BODY_DATE_SIZE: f32 = 15.0;
const BODY_DATE_GAP: f32 = 4.0;
const TODO_NEXT_GAP: f32 = 6.0;
const DOT: f32 = 7.0;
const DOT_GAP: f32 = 7.0;
const WEATHER_TEMP_SIZE: f32 = 28.0;
const WEATHER_GAP: f32 = 4.0;
const TRANSPORT_BTN: f32 = 34.0;
const TRANSPORT_ICON: f32 = 18.0;
const TRANSPORT_GAP: f32 = 6.0;
const MAX_LINES: usize = 3;

// State washes and borders, the CSS color-mix shares.
const ACTIVE_WASH: f32 = 0.18;
const ACTIVE_BORDER_MIX: f32 = 0.34;
const HOVER_BORDER_MIX: f32 = 0.40;
const DANGER_HOVER_MIX: f32 = 0.44;
const CONFIRM_WASH: f32 = 0.20;
const CONFIRM_BORDER_MIX: f32 = 0.50;
const PRESSED_OPACITY: f32 = 0.85;

/// A forgotten prompt must not fire on a later stray press.
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);
/// Ticks are one second; Now Playing changes out of band, the task rotates.
const TICK: Duration = Duration::from_secs(1);
const MEDIA_POLL_TICKS: u64 = 2;
const TASK_ROTATE_TICKS: u64 = 3;

const NOTICE_OK: f32 = 1.2;
const NOTICE_INFO: f32 = 2.2;
const NOTICE_ERROR: f32 = 1.6;
/// Long enough to read a config error, which is longer than a toast.
const NOTICE_WARNING: f32 = 5.0;

const PLACEHOLDER: &str = "--";
const TEMP_PLACEHOLDER: &str = "--°";
const RAIN_PLACEHOLDER: &str = "--%";
const IDLE_TITLE: &str = "Nothing playing";
const ALL_CLEAR: &str = "All clear";
const DONE_TODAY: &str = "done today";
const LUNAR: &str = "Lunar";
const LUNAR_LEAP: &str = "Lunar leap";
const UPTIME: &str = "Uptime";
const CONFIRM_FALLBACK: &str = "Confirm?";
const PLAYPAUSE: &str = "playpause";
const NEXT: &str = "next";
const PREVIOUS: &str = "previous";
/// The todo store keys days like this.
const DAY_KEY: &str = "%Y-%m-%d";
const TIME_FORMAT: &str = "%H:%M";
/// "Mon, Oct 6", what the webview's short locale date gives.
const DATE_FORMAT: &str = "%a, %b %-d";

/// A line for the launcher's banner.
#[derive(Clone)]
pub struct Notice {
    pub text: String,
    pub tone: Tone,
    pub seconds: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Success,
    Info,
    Warning,
    Error,
}

impl EventEmitter<Notice> for Launchpad {}

/// What an adapter last said about its control.
#[derive(Default)]
struct Control {
    wired: bool,
    on: bool,
    /// Battery's percent.
    value: Option<String>,
    charging: bool,
    reason: Option<String>,
}

#[derive(Default)]
struct TodoToday {
    done: usize,
    total: usize,
    open: Vec<String>,
}

/// What the L slot shows, by priority. Pomo joins when the command screens
/// land in M5.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Todo,
    Clock,
}

/// The layout survives the window, so a summon never waits on the file.
static LAYOUT: Mutex<Option<Arc<LayoutPayload>>> = Mutex::new(None);
/// A broken drawing is reported once per process, not once per summon.
static WARNED: AtomicBool = AtomicBool::new(false);

fn cached_layout() -> Option<Arc<LayoutPayload>> {
    LAYOUT.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

pub struct Launchpad {
    layout: Option<Arc<LayoutPayload>>,
    controls: HashMap<String, Control>,
    uptime: Option<String>,
    weather: Option<WeatherSnapshot>,
    media: Option<NowPlayingSnapshot>,
    lunar: Option<LunarDate>,
    lunar_day: Option<NaiveDate>,
    todo: TodoToday,
    task_cursor: usize,
    /// None until the first read; a tile with no entry after that is hidden.
    custom: Option<HashMap<String, TileValue>>,
    /// Decoded user icons by data URL.
    images: HashMap<String, Option<Arc<Image>>>,
    /// The destructive tile awaiting its second press.
    confirm: Option<String>,
    _confirm_timer: Option<Task<()>>,
    applying: bool,
    /// Bumped on every refresh and press, so a late read cannot clobber what
    /// the user just did.
    token: u64,
    shown: bool,
    minute: u32,
    _ticker: Task<()>,
}

impl Launchpad {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            let mut tick: u64 = 0;
            loop {
                cx.background_executor().timer(TICK).await;
                tick += 1;
                if this.update(cx, |this, cx| this.tick(tick, cx)).is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            layout: cached_layout(),
            controls: HashMap::new(),
            uptime: None,
            weather: None,
            media: None,
            lunar: None,
            lunar_day: None,
            todo: TodoToday::default(),
            task_cursor: 0,
            custom: None,
            images: HashMap::new(),
            confirm: None,
            _confirm_timer: None,
            applying: false,
            token: 0,
            shown: true,
            minute: Local::now().minute(),
            _ticker: ticker,
        };
        if this.layout.is_some() {
            this.refresh_all(cx);
        }
        this.load_layout(cx);
        this
    }

    /// Whether the bento is on screen. Hidden, it stops polling and drops an
    /// armed confirm; shown again, it re-reads everything.
    pub fn set_shown(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown == self.shown {
            return;
        }
        self.shown = shown;
        if shown {
            self.refresh_all(cx);
        } else {
            self.clear_confirm(cx);
        }
    }

    /// Alt+<char>: the tile owning the letter fires. False when none does.
    pub fn mnemonic(&mut self, ch: char, cx: &mut Context<Self>) -> bool {
        let id = self.tiles().iter().find_map(|tile| {
            tile.mnemonic
                .filter(|m| m.eq_ignore_ascii_case(&ch))
                .map(|_| tile.action_id.clone())
        });
        match id {
            Some(id) => self.activate(&id, cx),
            None => false,
        }
    }

    fn tiles(&self) -> &[LaunchpadTile] {
        self.layout.as_ref().map_or(&[], |l| l.tiles.as_slice())
    }

    fn tile(&self, id: &str) -> Option<&LaunchpadTile> {
        self.tiles().iter().find(|t| t.action_id == id)
    }

    fn has_role(&self, role: TileRole) -> bool {
        self.tiles().iter().any(|t| t.role == role)
    }

    fn control(&self, id: &str) -> Option<&Control> {
        self.controls.get(id)
    }

    fn slot(&self) -> Slot {
        if self.todo.open.is_empty() {
            Slot::Clock
        } else {
            Slot::Todo
        }
    }

    // --- Background reads ---------------------------------------------------

    /// Run `work` off the UI thread and apply its answer, unless a refresh or
    /// a press has moved on since.
    fn fetch<T: Send + 'static>(
        &self,
        cx: &mut Context<Self>,
        work: impl FnOnce() -> T + Send + 'static,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        let token = self.token;
        cx.spawn(async move |this, cx| {
            let value = cx.background_executor().spawn(async move { work() }).await;
            let _ = this.update(cx, |this, cx| {
                if this.token == token {
                    apply(this, value, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The drawing may have changed since the cache was filled.
    fn load_layout(&mut self, cx: &mut Context<Self>) {
        let token = self.token;
        cx.spawn(async move |this, cx| {
            let fresh = cx
                .background_executor()
                .spawn(async move {
                    let layout = Arc::new(qactions::launchpad_layout());
                    let warnings = if WARNED.swap(true, Ordering::Relaxed) {
                        Vec::new()
                    } else {
                        qactions::launchpad_warnings()
                    };
                    (layout, warnings)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let (layout, warnings) = fresh;
                *LAYOUT.lock().unwrap_or_else(|p| p.into_inner()) = Some(layout.clone());
                let first = this.layout.is_none();
                if first || this.layout.as_deref() != Some(&layout) {
                    this.layout = Some(layout);
                    this.clear_confirm(cx);
                    if first || this.token == token {
                        this.refresh_all(cx);
                    }
                    cx.notify();
                }
                if let Some((first, rest)) = warnings.split_first() {
                    let text = if rest.is_empty() {
                        first.clone()
                    } else {
                        format!("{first} (+{} more)", rest.len())
                    };
                    this.notify(text, Tone::Warning, NOTICE_WARNING, cx);
                }
            });
        })
        .detach();
    }

    fn refresh_all(&mut self, cx: &mut Context<Self>) {
        self.token += 1;
        self.refresh_lunar();
        let ids: Vec<(String, TileRole)> = self
            .tiles()
            .iter()
            .map(|t| (t.action_id.clone(), t.role))
            .collect();
        let mut custom = false;
        for (id, role) in ids {
            match role {
                TileRole::Toggle | TileRole::Info | TileRole::Action => {
                    self.refresh_control(id, cx)
                }
                TileRole::Custom => custom = true,
                TileRole::Weather | TileRole::Media | TileRole::Slot => {}
            }
        }
        if self.has_role(TileRole::Weather) {
            self.fetch(cx, weather::weather_current, |this, reading, _| {
                // The feed had nothing: keep the last reading over a blank.
                if reading.is_some() {
                    this.weather = reading;
                }
            });
        }
        if self.has_role(TileRole::Media) {
            self.refresh_media(cx);
        }
        if self.has_role(TileRole::Slot) {
            self.fetch(
                cx,
                || todo::todo_list().unwrap_or_default(),
                |this, tasks, _| {
                    let today = Local::now().format(DAY_KEY).to_string();
                    let mine: Vec<_> = tasks.into_iter().filter(|t| t.due_date == today).collect();
                    this.todo = TodoToday {
                        done: mine.iter().filter(|t| t.done).count(),
                        total: mine.len(),
                        open: mine
                            .into_iter()
                            .filter(|t| !t.done)
                            .map(|t| t.name)
                            .collect(),
                    };
                    this.task_cursor = 0;
                },
            );
        }
        if custom {
            self.refresh_custom(cx);
        }
    }

    fn refresh_control(&mut self, id: String, cx: &mut Context<Self>) {
        let keys: Vec<String> = if id == action_id::BATTERY {
            vec![BATTERY_CHARGING_INFO_KEY.to_string()]
        } else {
            Vec::new()
        };
        let read_id = id.clone();
        self.fetch(
            cx,
            move || qactions::quick_action_state(&read_id, &keys),
            move |this, status, cx| {
                let control = this.controls.entry(id.clone()).or_default();
                control.wired = true;
                control.reason = None;
                match status.state {
                    ActionState::On => control.on = true,
                    ActionState::Off => control.on = false,
                    ActionState::Value { value } => control.value = Some(value),
                    ActionState::Unavailable { reason } => {
                        control.wired = false;
                        control.reason = Some(reason);
                    }
                }
                control.charging = matches!(
                    status.info.get(BATTERY_CHARGING_INFO_KEY),
                    Some(InfoValue::Text { text }) if text == BATTERY_CHARGING_INFO_TEXT
                );
                // No battery: the tile shows uptime instead of a dead dash.
                if !control.wired && id == action_id::BATTERY {
                    this.fetch(cx, sysinfo::system_uptime, |this, uptime, _| {
                        this.uptime = uptime;
                    });
                }
            },
        );
    }

    fn refresh_media(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, nowplaying::now_playing_current, |this, media, _| {
            this.media = media;
        });
    }

    /// Two reads on purpose: the cache answers at once so the strip never
    /// waits on a command, then stale commands run and the values come again.
    fn refresh_custom(&mut self, cx: &mut Context<Self>) {
        self.fetch(cx, qactions::launchpad_tile_values, |this, values, _| {
            this.custom = Some(values);
        });
        self.fetch(
            cx,
            || {
                let (refreshed, errors) = qactions::refresh_launchpad_tiles();
                let values = (refreshed > 0).then(qactions::launchpad_tile_values);
                (values, errors)
            },
            |this, (values, errors), cx| {
                for message in errors {
                    this.notify(message, Tone::Error, NOTICE_ERROR, cx);
                }
                if values.is_some() {
                    this.custom = values;
                }
            },
        );
    }

    /// Pure arithmetic, so it runs inline; the date only changes at midnight.
    fn refresh_lunar(&mut self) {
        let now = Local::now();
        let day = now.date_naive();
        if self.lunar_day == Some(day) {
            return;
        }
        let tz = now.offset().fix().local_minus_utc() as f64 / 3600.0;
        self.lunar = Some(lunar::lunar_date(
            day.year() as i64,
            day.month() as i64,
            day.day() as i64,
            tz,
        ));
        self.lunar_day = Some(day);
    }

    fn tick(&mut self, tick: u64, cx: &mut Context<Self>) {
        let minute = Local::now().minute();
        if minute != self.minute {
            self.minute = minute;
            self.refresh_lunar();
            cx.notify();
        }
        if !self.shown {
            return;
        }
        if tick.is_multiple_of(MEDIA_POLL_TICKS) && self.has_role(TileRole::Media) {
            self.refresh_media(cx);
        }
        if tick.is_multiple_of(TASK_ROTATE_TICKS)
            && self.slot() == Slot::Todo
            && self.todo.open.len() > 1
        {
            self.task_cursor += 1;
            cx.notify();
        }
    }

    // --- Activation -------------------------------------------------------------

    /// One path for a click and a mnemonic. Toggles flip, buttons fire, a
    /// destructive one arms first, Now Playing plays or pauses, a user tile
    /// runs its command. An unwired tile says why it cannot.
    fn activate(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(tile) = self.tile(id).cloned() else {
            return false;
        };
        if self.confirm.as_deref().is_some_and(|armed| armed != id) {
            self.clear_confirm(cx);
        }
        if tile.role == TileRole::Media {
            self.transport(PLAYPAUSE, cx);
            return true;
        }
        if tile.confirm.is_some() && self.confirm.as_deref() != Some(id) {
            self.arm_confirm(id.to_string(), cx);
            return true;
        }
        self.clear_confirm(cx);

        match tile.role {
            TileRole::Custom => {
                if tile.pressable {
                    let name = tile.action_id.clone();
                    self.fetch(
                        cx,
                        move || qactions::press_launchpad_tile(&name),
                        |this, error, cx| match error {
                            Some(message) => this.notify(message, Tone::Error, NOTICE_ERROR, cx),
                            None => this.refresh_custom(cx),
                        },
                    );
                }
            }
            TileRole::Toggle | TileRole::Action => {
                let Some(control) = self.control(id) else {
                    return true;
                };
                if !control.wired {
                    if let Some(reason) = control.reason.clone() {
                        self.notify(reason, Tone::Info, NOTICE_ERROR, cx);
                    }
                    return true;
                }
                // An off caption means the button flips state (Mic) rather
                // than firing once.
                if tile.role == TileRole::Toggle || tile.off_label.is_some() {
                    let target = !control.on;
                    self.apply(&tile, ActionIntent::SetOn(target), cx);
                } else {
                    self.apply(&tile, ActionIntent::Run, cx);
                }
            }
            TileRole::Info | TileRole::Weather | TileRole::Slot | TileRole::Media => {}
        }
        true
    }

    /// Optimistic: the tile shows the target now, the adapter's answer and a
    /// re-read follow. One at a time, so a double press cannot race.
    fn apply(&mut self, tile: &LaunchpadTile, intent: ActionIntent, cx: &mut Context<Self>) {
        if self.applying {
            return;
        }
        self.applying = true;
        let id = tile.action_id.clone();
        let title = tile.title.clone();
        let reads_back = if let ActionIntent::SetOn(target) = intent {
            self.controls.entry(id.clone()).or_default().on = target;
            true
        } else {
            false
        };
        self.token += 1;
        cx.notify();
        let apply_id = id.clone();
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move { qactions::quick_action_apply(&apply_id, intent) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.applying = false;
                this.notify_outcome(&title, outcome, cx);
                if reads_back {
                    this.token += 1;
                    this.refresh_control(id, cx);
                }
            });
        })
        .detach();
    }

    fn transport(&mut self, command: &'static str, cx: &mut Context<Self>) {
        let player = self.media.as_ref().and_then(|m| m.player.clone());
        if command == PLAYPAUSE {
            // Flip now, roll back unless delivered. No re-read: MPRIS lags the
            // command and would flip it back for a frame.
            let Some(media) = self.media.as_mut() else {
                cx.background_executor()
                    .spawn(async move {
                        nowplaying::now_playing_command(PLAYPAUSE, player.as_deref());
                    })
                    .detach();
                return;
            };
            let was = media.is_playing;
            media.is_playing = !was;
            cx.notify();
            cx.spawn(async move |this, cx| {
                let delivered =
                    cx.background_executor()
                        .spawn(async move {
                            nowplaying::now_playing_command(PLAYPAUSE, player.as_deref())
                        })
                        .await;
                if !delivered {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(media) = this.media.as_mut() {
                            media.is_playing = was;
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
            return;
        }
        // Next or previous: re-read so the new track shows without the poll.
        self.fetch(
            cx,
            move || {
                nowplaying::now_playing_command(command, player.as_deref());
                nowplaying::now_playing_current()
            },
            |this, media, _| this.media = media,
        );
    }

    fn arm_confirm(&mut self, id: String, cx: &mut Context<Self>) {
        self.confirm = Some(id.clone());
        self._confirm_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CONFIRM_TIMEOUT).await;
            let _ = this.update(cx, |this, cx| {
                if this.confirm.as_deref() == Some(id.as_str()) {
                    this.clear_confirm(cx);
                }
            });
        }));
        cx.notify();
    }

    fn clear_confirm(&mut self, cx: &mut Context<Self>) {
        if self.confirm.take().is_some() {
            self._confirm_timer = None;
            cx.notify();
        }
    }

    fn notify(&self, text: String, tone: Tone, seconds: f32, cx: &mut Context<Self>) {
        cx.emit(Notice {
            text,
            tone,
            seconds,
        });
    }

    fn notify_outcome(&self, title: &str, outcome: ActionOutcome, cx: &mut Context<Self>) {
        match outcome {
            ActionOutcome::Ok { banner } => self.notify(
                banner.unwrap_or_else(|| format!("{title} done")),
                Tone::Success,
                NOTICE_OK,
                cx,
            ),
            ActionOutcome::NeedsPermission { message } => {
                self.notify(message, Tone::Info, NOTICE_INFO, cx)
            }
            ActionOutcome::Failed { message } => {
                self.notify(message, Tone::Error, NOTICE_ERROR, cx)
            }
        }
    }

    // --- Rendering --------------------------------------------------------------

    /// The grid's height and its tiles, each placed and animated in. The
    /// caller owns the container, so the blur region is marked where every
    /// other card is.
    pub fn tiles_in(&mut self, th: &Theme, cx: &mut Context<Self>) -> (f32, Vec<AnyElement>) {
        let Some(layout) = self.layout.clone() else {
            return (0.0, Vec::new());
        };
        let gap = th.inner_gap;
        let cols = f32::from(layout.columns.max(1));
        let rows = f32::from(layout.rows.max(1));
        let inner_w = theme::WINDOW_W - 2.0 * theme::CONTENT_PADDING;
        let cell_w = (inner_w - gap * (cols - 1.0)) / cols;
        let grid_h = ROW_H * rows + gap * (rows - 1.0);

        let visible: Vec<usize> = (0..layout.tiles.len())
            .filter(|&i| !self.hidden(&layout.tiles[i]))
            .collect();
        let mut tiles = Vec::with_capacity(visible.len());
        for i in visible {
            let tile = &layout.tiles[i];
            let x = f32::from(tile.col) * (cell_w + gap);
            let y = f32::from(tile.row) * (ROW_H + gap);
            let span_w = f32::from(tile.col_span);
            let span_h = f32::from(tile.row_span);
            let w = span_w * cell_w + (span_w - 1.0) * gap;
            let h = span_h * ROW_H + (span_h - 1.0) * gap;

            let delay = motion::tile_delay(i);
            let duration = Duration::from_millis(motion::TILE_MS);
            let total = delay + duration;

            tiles.push(
                self.tile_face(i, tile, th, cx)
                    .with_animation(("tile", i), Animation::new(total), move |tile, progress| {
                        let t = motion::spring(motion::staggered(progress, total, delay, duration));
                        let scale = motion::TILE_SCALE + (1.0 - motion::TILE_SCALE) * t;
                        let (sw, sh) = (w * scale, h * scale);
                        tile.opacity(t.min(1.0))
                            .left(px(x + (w - sw) / 2.0))
                            .top(px(y + (h - sh) / 2.0 + motion::TILE_RISE * (1.0 - t)))
                            .w(px(sw))
                            .h(px(sh))
                    })
                    .into_any_element(),
            );
        }
        (grid_h, tiles)
    }

    /// A user tile whose command printed nothing has nothing to show.
    fn hidden(&self, tile: &LaunchpadTile) -> bool {
        tile.role == TileRole::Custom
            && tile.has_value
            && self
                .custom
                .as_ref()
                .is_some_and(|values| !values.contains_key(&tile.action_id))
    }

    /// The card and its state colours; the content comes from the role.
    fn tile_face(
        &mut self,
        index: usize,
        tile: &LaunchpadTile,
        th: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = tile.action_id.clone();
        let confirming = self.confirm.as_deref() == Some(id.as_str());
        let control = self.control(&id);
        let on = match tile.role {
            TileRole::Toggle => control.is_some_and(|c| c.wired && c.on),
            TileRole::Custom => self
                .custom
                .as_ref()
                .and_then(|values| values.get(&id))
                .and_then(|v| v.state.as_deref())
                .is_some_and(|s| s.eq_ignore_ascii_case("on")),
            _ => false,
        };
        let danger = tile.confirm.is_some();

        let face = th.tile_face();
        let (bg, border) = if confirming {
            (
                theme::wash(face, th.danger, CONFIRM_WASH),
                theme::mix(th.danger, th.border, CONFIRM_BORDER_MIX),
            )
        } else if on {
            (
                theme::wash(face, th.accent, ACTIVE_WASH),
                theme::mix(th.accent, th.border, ACTIVE_BORDER_MIX),
            )
        } else {
            (face, th.border)
        };
        let hover_border = if danger {
            theme::mix(th.danger, th.border, DANGER_HOVER_MIX)
        } else {
            theme::mix(th.accent, th.border, HOVER_BORDER_MIX)
        };

        let content = match tile.role {
            TileRole::Slot => self.slot_tile(th),
            TileRole::Toggle => self.toggle_tile(tile, on, th),
            TileRole::Info => self.info_tile(tile, th),
            TileRole::Weather => self.weather_tile(tile, th),
            TileRole::Action => self.action_tile(tile, confirming, th),
            TileRole::Media => self.media_tile(index, th, cx),
            TileRole::Custom => self.custom_tile(tile, confirming, th),
        };

        let pressable = tile.pressable && tile.role != TileRole::Media;
        div()
            .id(("tile", index))
            .absolute()
            .flex()
            .overflow_hidden()
            .bg(bg)
            .border(px(th.border_thickness))
            .border_color(border)
            .shadow(th.card_shadow())
            .rounded(px(th.tile_radius()))
            .when(pressable, |el| {
                el.hover(|s| s.border_color(hover_border))
                    .active(|s| s.opacity(PRESSED_OPACITY))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.activate(&id, cx);
                    }))
            })
            .child(content)
    }

    /// The 2x2 slot: an icon badge and a header readout up top, the winning
    /// body below. Todo keeps a compact clock in the corner; the Clock slot
    /// owns the big time and puts the lunar date in the corner instead.
    fn slot_tile(&self, th: &Theme) -> Div {
        let now = Local::now();
        let slot = self.slot();
        let badge = div()
            .size(px(SLOT_BADGE))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(th.control_fill)
            .rounded(px(th.bar_radius()))
            .child(glyph(
                match slot {
                    Slot::Todo => glyphs::LIST_CHECKS,
                    Slot::Clock => glyphs::CLOCK,
                },
                ICON_SLOT,
                th.text_secondary,
            ));
        let lunar_short = self
            .lunar
            .as_ref()
            .map(|l| format!("{}/{}", l.day, l.month));
        let lunar_word = match self.lunar.as_ref() {
            Some(l) if l.leap => LUNAR_LEAP,
            _ => LUNAR,
        };
        let corner = div().flex().flex_col().items_end();
        let corner = match slot {
            Slot::Clock => corner
                .child(
                    mono(
                        lunar_short.unwrap_or_else(|| PLACEHOLDER.to_string()),
                        HEAD_LUNAR_SIZE,
                        th,
                    )
                    .text_color(th.text),
                )
                .child(small(lunar_word, STATE_SIZE, th.text_muted)),
            Slot::Todo => corner
                .child(
                    mono(now.format(TIME_FORMAT).to_string(), HEAD_TIME_SIZE, th)
                        .text_color(th.text),
                )
                .child(small(
                    now.format(DATE_FORMAT).to_string(),
                    STATE_SIZE,
                    th.text_muted,
                ))
                .when_some(lunar_short, |col, short| {
                    col.child(
                        mono(format!("{short} {lunar_word}"), HEAD_LUNAR_LINE_SIZE, th)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(th.text_muted),
                    )
                }),
        };

        let body = match slot {
            Slot::Clock => div()
                .flex()
                .flex_col()
                .child(
                    mono(now.format(TIME_FORMAT).to_string(), BODY_TIME_SIZE, th)
                        .text_color(th.text),
                )
                .child(
                    small(
                        now.format(DATE_FORMAT).to_string(),
                        BODY_DATE_SIZE,
                        th.text_muted,
                    )
                    .mt(px(BODY_DATE_GAP)),
                ),
            Slot::Todo => {
                let next = self
                    .todo
                    .open
                    .get(self.task_cursor % self.todo.open.len().max(1))
                    .cloned()
                    .unwrap_or_else(|| ALL_CLEAR.to_string());
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(TODO_NEXT_GAP))
                            .child(
                                mono(
                                    format!("{}/{}", self.todo.done, self.todo.total),
                                    BODY_TIME_SIZE,
                                    th,
                                )
                                .text_color(th.text),
                            )
                            .child(small(DONE_TODAY, BODY_DATE_SIZE, th.text_muted)),
                    )
                    .child(
                        div()
                            .mt(px(TODO_NEXT_GAP))
                            .flex()
                            .items_center()
                            .gap(px(DOT_GAP))
                            .child(
                                div()
                                    .size(px(DOT))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(th.accent),
                            )
                            .child(
                                small(next, LABEL_SMALL, th.text_secondary)
                                    .flex_1()
                                    .min_w_0()
                                    .truncate(),
                            ),
                    )
            }
        };

        div()
            .flex_1()
            .min_w_0()
            .p(px(SLOT_PADDING))
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(HEAD_GAP))
                    .child(badge)
                    .child(div().flex_1())
                    .child(corner),
            )
            .child(body)
    }

    fn toggle_tile(&self, tile: &LaunchpadTile, on: bool, th: &Theme) -> Div {
        let icon = match tile.action_id.as_str() {
            action_id::THEME if !on => glyphs::SUN,
            id => glyph_for(id),
        };
        let state = if on {
            tile.on_label.as_deref().unwrap_or("On")
        } else {
            tile.off_label.as_deref().unwrap_or("Off")
        };
        let state_colour = if on { th.accent } else { th.text_muted };
        div()
            .flex_1()
            .min_w_0()
            .p(px(theme::TILE_PADDING))
            .flex()
            .items_center()
            .gap(px(TILE_GAP))
            .child(glyph(
                icon,
                ICON_TOGGLE,
                if on { th.accent } else { th.text_secondary },
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(px(TEXT_GAP))
                    .child(
                        div()
                            .text_size(px(LABEL_SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(th.text)
                            .child(mnemonic_text(&tile.title, tile.mnemonic, th)),
                    )
                    .child(
                        small(state.to_string(), STATE_SIZE, state_colour)
                            .when(on, |el| el.font_weight(FontWeight::MEDIUM)),
                    ),
            )
    }

    /// Battery, or uptime on a machine without one.
    fn info_tile(&self, tile: &LaunchpadTile, th: &Theme) -> Div {
        let control = self.control(&tile.action_id);
        let (caption, value, icon) = match control.and_then(|c| c.value.clone()) {
            Some(value) => (
                tile.title.clone(),
                value,
                if control.is_some_and(|c| c.charging) {
                    glyphs::BATTERY_CHARGING
                } else {
                    glyph_for(&tile.action_id)
                },
            ),
            None => match (control.is_some_and(|c| !c.wired), self.uptime.clone()) {
                (true, Some(uptime)) => (UPTIME.to_string(), uptime, glyph_for(&tile.action_id)),
                _ => (
                    tile.title.clone(),
                    PLACEHOLDER.to_string(),
                    glyph_for(&tile.action_id),
                ),
            },
        };
        div()
            .flex_1()
            .min_w_0()
            .p(px(theme::TILE_PADDING))
            .flex()
            .items_center()
            .gap(px(TILE_GAP))
            .child(glyph(icon, ICON_INFO, th.text_secondary))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(px(INFO_TEXT_GAP))
                    .child(caps(caption, th))
                    .child(mono(value, theme::TILE_VALUE_SIZE, th).text_color(th.text)),
            )
    }

    fn weather_tile(&self, tile: &LaunchpadTile, th: &Theme) -> Div {
        let w = self.weather.as_ref();
        let icon = w.map_or(glyphs::CLOUD_SUN, |w| weather_glyph(&w.symbol));
        let temp = w.map_or(TEMP_PLACEHOLDER.to_string(), |w| w.temperature.clone());
        let condition = w.map_or(tile.title.clone(), |w| w.condition.clone());
        let range = match w {
            Some(w) => format!("H {}   L {}", w.high, w.low),
            None => format!("H {TEMP_PLACEHOLDER}   L {TEMP_PLACEHOLDER}"),
        };
        let rain = w
            .and_then(|w| w.rain_chance.clone())
            .unwrap_or_else(|| RAIN_PLACEHOLDER.to_string());
        div()
            .flex_1()
            .min_w_0()
            .p(px(SLOT_PADDING))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(WEATHER_GAP))
            .child(glyph(icon, ICON_WEATHER, th.text_secondary))
            .child(mono(temp, WEATHER_TEMP_SIZE, th).text_color(th.text))
            .child(caps(condition, th))
            .child(small(range, STATE_SIZE, th.text_muted))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(WEATHER_GAP))
                    .text_size(px(STATE_SIZE))
                    .text_color(th.text_muted)
                    .child(glyph(glyphs::DROPLET, ICON_RAIN, th.text_muted))
                    .child(rain),
            )
    }

    /// Mic, Screensaver, Restart, Shut Down: a glyph over a name. Mic's
    /// glyph says muted in amber; the destructive two wear the danger colour
    /// and swap their name for the question while armed.
    fn action_tile(&self, tile: &LaunchpadTile, confirming: bool, th: &Theme) -> Div {
        let control = self.control(&tile.action_id);
        let flips = tile.off_label.is_some();
        let muted = flips && control.is_some_and(|c| c.wired && !c.on);
        let danger = tile.confirm.is_some();
        let icon = if flips {
            if muted { glyphs::MIC_OFF } else { glyphs::MIC }
        } else {
            glyph_for(&tile.action_id)
        };
        let tint = if confirming || danger {
            th.danger
        } else if muted {
            th.warning
        } else {
            th.text_secondary
        };
        let label = if confirming {
            StyledText::new(SharedString::from(
                tile.confirm
                    .clone()
                    .unwrap_or_else(|| CONFIRM_FALLBACK.to_string()),
            ))
        } else {
            mnemonic_text(&tile.title, tile.mnemonic, th)
        };
        action_body(
            glyph(icon, ICON_ACTION, tint).into_any_element(),
            label,
            confirming,
            th,
        )
    }

    /// Track and artist beside a previous, play or pause, next transport.
    /// The tile itself is inert: the buttons and Alt+P own the actions.
    fn media_tile(&self, index: usize, th: &Theme, cx: &mut Context<Self>) -> Div {
        let media = self.media.as_ref().filter(|m| !m.title.is_empty());
        let title = media.map_or(IDLE_TITLE.to_string(), |m| m.title.clone());
        let subtitle = media
            .and_then(|m| m.artist.clone().or_else(|| m.app.clone()))
            .unwrap_or_default();
        let playing = self.media.as_ref().is_some_and(|m| m.is_playing);

        let button = |key: &'static str, icon: &'static str, command: &'static str, colour| {
            div()
                .id((key, index))
                .size(px(TRANSPORT_BTN))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(th.control_radius()))
                .hover(|s| s.bg(th.control_fill))
                .active(|s| s.opacity(PRESSED_OPACITY))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.transport(command, cx);
                }))
                .child(glyph(icon, TRANSPORT_ICON, colour))
        };

        div()
            .flex_1()
            .min_w_0()
            .px(px(MEDIA_GAP))
            .py(px(theme::TILE_PADDING))
            .flex()
            .items_center()
            .gap(px(MEDIA_GAP))
            .child(glyph(glyphs::MUSIC, ICON_INFO, th.text_secondary))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(TEXT_GAP))
                    .child(
                        div()
                            .text_size(px(LABEL_SIZE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if media.is_some() {
                                th.text
                            } else {
                                th.text_muted
                            })
                            .truncate()
                            .child(title),
                    )
                    .when(!subtitle.is_empty(), |col| {
                        col.child(small(subtitle, STATE_SIZE, th.text_muted).truncate())
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(TRANSPORT_GAP))
                    .child(button(
                        "prev",
                        glyphs::SKIP_BACK,
                        PREVIOUS,
                        th.text_secondary,
                    ))
                    .child(
                        button(
                            "play",
                            if playing { glyphs::PAUSE } else { glyphs::PLAY },
                            PLAYPAUSE,
                            th.text,
                        )
                        .bg(th.control_fill)
                        .border(px(th.border_thickness))
                        .border_color(th.border),
                    )
                    .child(button(
                        "next",
                        glyphs::SKIP_FORWARD,
                        NEXT,
                        th.text_secondary,
                    )),
            )
    }

    /// A tile from `~/.look/super-actions.toml`: Battery's anatomy when it has a
    /// reading, Mic's when it only acts. How much shows is how big it was drawn.
    fn custom_tile(&mut self, tile: &LaunchpadTile, confirming: bool, th: &Theme) -> Div {
        let value = self
            .custom
            .as_ref()
            .and_then(|values| values.get(&tile.action_id))
            .cloned();
        let icon_name = value
            .as_ref()
            .and_then(|v| v.icon.clone())
            .or_else(|| tile.icon.clone());

        if !tile.has_value {
            let icon = self
                .custom_icon(icon_name.as_deref(), ICON_ACTION, th.text_secondary)
                .unwrap_or_else(|| {
                    glyph(glyphs::POWER, ICON_ACTION, th.text_secondary).into_any_element()
                });
            let label = if confirming {
                StyledText::new(SharedString::from(
                    tile.confirm
                        .clone()
                        .unwrap_or_else(|| CONFIRM_FALLBACK.to_string()),
                ))
            } else {
                mnemonic_text(&tile.title, tile.mnemonic, th)
            };
            return action_body(icon, label, confirming, th);
        }

        let roomy = tile.row_span > 1 || tile.col_span > 1;
        let caption = value.as_ref().and_then(|v| v.caption.clone());
        // The command's caption wins over the name, as Weather shows the
        // condition. Unless the tile has a key: that letter is in the name.
        let name = if confirming {
            StyledText::new(SharedString::from(
                tile.confirm
                    .clone()
                    .unwrap_or_else(|| CONFIRM_FALLBACK.to_string())
                    .to_uppercase(),
            ))
        } else if tile.mnemonic.is_none() && caption.is_some() {
            StyledText::new(SharedString::from(
                caption.clone().unwrap_or_default().to_uppercase(),
            ))
        } else {
            mnemonic_text(&tile.title.to_uppercase(), tile.mnemonic, th)
        };
        let name = div()
            .text_size(px(theme::CAPTION_SIZE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if confirming { th.danger } else { th.text_muted })
            .truncate()
            .child(name);
        let reading = mono(
            value
                .as_ref()
                .map_or(PLACEHOLDER.to_string(), |v| v.value.clone()),
            theme::TILE_VALUE_SIZE,
            th,
        )
        .text_color(th.text);
        let icon_size = if roomy { ICON_CUSTOM_HEAD } else { ICON_INFO };
        let mut icon = self.custom_icon(icon_name.as_deref(), icon_size, th.text_secondary);

        let mut text = div().flex().flex_col().min_w_0().gap(px(TEXT_GAP));
        if roomy {
            // Only the name shares the icon's row; the reading starts at the
            // tile's edge rather than in a gutter the icon opened.
            text = text.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(CUSTOM_HEAD_GAP))
                    .min_w_0()
                    .children(icon.take())
                    .child(name),
            );
        } else {
            text = text.child(name);
        }
        text = text.child(reading);
        if roomy {
            if let (Some(caption), Some(_)) = (caption, tile.mnemonic) {
                text = text.child(caps(caption, th).truncate());
            }
            if let Some(v) = value.as_ref() {
                text = text.children(
                    v.lines
                        .iter()
                        .take(MAX_LINES)
                        .map(|line| small(line.clone(), STATE_SIZE, th.text_secondary).truncate()),
                );
            }
        }

        let row = div()
            .flex_1()
            .min_w_0()
            .p(px(theme::TILE_PADDING))
            .flex()
            .items_start()
            .gap(px(TILE_GAP))
            .overflow_hidden();
        if roomy {
            row.child(text)
        } else {
            row.children(icon).child(text)
        }
    }

    /// A user icon: one of the shell's glyph names, or a file the backend
    /// inlined as a data URL. An SVG goes through the asset source so it takes
    /// the tile's colour like the glyphs beside it; a raster draws as it is.
    fn custom_icon(&mut self, name: Option<&str>, size: f32, colour: Rgba) -> Option<AnyElement> {
        let name = name?;
        if let Some(path) = named_glyph(name) {
            return Some(glyph(path, size, colour).into_any_element());
        }
        if name.starts_with(glyphs::INLINE_SVG_PREFIX) {
            return Some(
                svg()
                    .path(SharedString::from(name.to_string()))
                    .size(px(size))
                    .text_color(colour)
                    .flex_shrink_0()
                    .into_any_element(),
            );
        }
        if !name.starts_with("data:") {
            return None;
        }
        let image = self
            .images
            .entry(name.to_string())
            .or_insert_with(|| icons::decode_data_url(name).map(Arc::new))
            .clone()?;
        Some(img(image).size(px(size)).into_any_element())
    }
}

/// The centred glyph-over-name body of an action tile.
fn action_body(icon: AnyElement, label: StyledText, confirming: bool, th: &Theme) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .p(px(theme::TILE_PADDING))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(ACTION_GAP))
        .child(icon)
        .child(
            div()
                .text_size(px(LABEL_SMALL))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if confirming { th.danger } else { th.text })
                .truncate()
                .child(label),
        )
}

fn glyph(path: &'static str, size: f32, colour: Rgba) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .text_color(colour)
        .flex_shrink_0()
}

/// The glyph a built-in control wears, keyed as `superactions.js` keys them.
fn glyph_for(action_id: &str) -> &'static str {
    match action_id {
        action_id::BLUETOOTH => glyphs::BLUETOOTH,
        action_id::WIFI => glyphs::WIFI,
        action_id::THEME => glyphs::MOON,
        action_id::KEEP_AWAKE => glyphs::COFFEE,
        action_id::BATTERY => glyphs::BATTERY,
        action_id::SCREENSAVER => glyphs::MONITOR,
        action_id::MIC => glyphs::MIC,
        action_id::RESTART => glyphs::REFRESH,
        action_id::SHUTDOWN => glyphs::POWER,
        _ => glyphs::POWER,
    }
}

/// A glyph a user tile may name: the built-in controls' names.
fn named_glyph(name: &str) -> Option<&'static str> {
    matches!(
        name,
        action_id::BLUETOOTH
            | action_id::WIFI
            | action_id::THEME
            | action_id::KEEP_AWAKE
            | action_id::BATTERY
            | action_id::SCREENSAVER
            | action_id::MIC
            | action_id::RESTART
            | action_id::SHUTDOWN
    )
    .then(|| glyph_for(name))
}

/// The backend's WMO condition key to a glyph.
fn weather_glyph(symbol: &str) -> &'static str {
    match symbol {
        "clear" => glyphs::SUN,
        "partly" => glyphs::CLOUD_SUN,
        "cloudy" => glyphs::CLOUD,
        "fog" => glyphs::CLOUD_FOG,
        "drizzle" => glyphs::CLOUD_DRIZZLE,
        "rain" | "showers" => glyphs::CLOUD_RAIN,
        "snow" => glyphs::CLOUD_SNOW,
        "thunder" => glyphs::CLOUD_LIGHTNING,
        _ => glyphs::CLOUD_SUN,
    }
}

/// The small-caps caption: Battery's label, the weather condition.
fn caps(text: String, th: &Theme) -> Div {
    div()
        .text_size(px(theme::CAPTION_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_muted)
        .child(text.to_uppercase())
}

/// A bold figure in the mono face.
fn mono(text: String, size: f32, th: &Theme) -> Div {
    div()
        .font_family(th.mono_family.clone())
        .text_size(px(size))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

fn small(text: impl Into<SharedString>, size: f32, colour: Rgba) -> Div {
    div()
        .text_size(px(size))
        .text_color(colour)
        .child(text.into())
}

/// The title with its Alt+<char> letter in the warning colour and bold,
/// as the macOS launchpad tints its Cmd+<char>. Plain when the letter is
/// not in the title.
fn mnemonic_text(title: &str, mnemonic: Option<char>, th: &Theme) -> StyledText {
    let text = StyledText::new(SharedString::from(title.to_string()));
    let Some(range) = mnemonic.and_then(|ch| mnemonic_range(title, ch)) else {
        return text;
    };
    text.with_highlights([(
        range,
        HighlightStyle {
            color: Some(theme::hsla_of(th.warning)),
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
    )])
}

/// Byte range of the first letter matching `ch`, case-insensitively.
fn mnemonic_range(title: &str, ch: char) -> Option<Range<usize>> {
    let wanted: Vec<char> = ch.to_lowercase().collect();
    title.char_indices().find_map(|(at, c)| {
        (c.to_lowercase().collect::<Vec<char>>() == wanted).then(|| at..at + c.len_utf8())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonic_finds_the_first_letter_either_case() {
        assert_eq!(mnemonic_range("Bluetooth", 'B'), Some(0..1));
        assert_eq!(mnemonic_range("Shut Down", 'd'), Some(5..6));
        assert_eq!(mnemonic_range("Wi-Fi", 'x'), None);
        // A multi-byte title still yields a byte range.
        assert_eq!(mnemonic_range("Ánh sáng", 's'), Some(5..6));
    }

    #[test]
    fn weather_symbols_all_resolve() {
        for key in [
            "clear", "partly", "cloudy", "fog", "drizzle", "rain", "showers", "snow", "thunder",
        ] {
            assert!(weather_glyph(key).starts_with("icons/"));
        }
        assert_eq!(weather_glyph("nonsense"), glyphs::CLOUD_SUN);
    }

    #[test]
    fn user_tiles_may_name_the_built_in_glyphs() {
        assert_eq!(named_glyph("battery"), Some(glyphs::BATTERY));
        assert_eq!(named_glyph("data:image/svg+xml;base64,"), None);
    }
}
