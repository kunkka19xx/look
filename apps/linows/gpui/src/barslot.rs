//! The slot at the right end of the top bar, holding what `bar_slot` says.
//! Read on each open; hidden in the compact layout and while translating.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{Animation, AnimationExt, Context, Div, SharedString, div, prelude::*, px};
use linows_backend::config::{self, ConfigUpdate};
use linows_backend::look_engine::bar_slot::{self, BarSlot};
use linows_backend::todo as store;
use linows_backend::{launch, search};

use crate::Shell;
use crate::bg;
use crate::config_list;
use crate::icons::IconStore;
use crate::launcher::Launcher;
use crate::motion;
use crate::running::{self, RunningApps};
use crate::theme::{self, Theme};
use crate::todo::Today;

const TODO_EMPTY: &str = "Plan today";
const TODO_ALL_DONE: &str = "done";
const TEXT_GAP: f32 = 8.0;
const APP_KIND: &str = "app";
const APP_USAGE: &str = "open_app";

/// No title when the index no longer has the app.
#[derive(Clone)]
pub struct Pin {
    pub id: String,
    pub title: Option<String>,
    path: String,
}

/// Kept for the process: the settings screen lists them too.
static PINS: Mutex<Vec<Pin>> = Mutex::new(Vec::new());

pub fn pins() -> std::sync::MutexGuard<'static, Vec<Pin>> {
    PINS.lock().unwrap_or_else(|p| p.into_inner())
}

fn pinned_in(entries: &HashMap<String, String>) -> String {
    entries
        .get(bar_slot::PINNED_APPS_KEY)
        .cloned()
        .unwrap_or_default()
}

fn pinned_value() -> String {
    pinned_in(&theme::entries())
}

pub fn is_pinned(id: &str) -> bool {
    config_list::parse(&pinned_value()).iter().any(|p| p == id)
}

#[derive(Debug, PartialEq, Eq)]
pub enum PinChange {
    Pinned,
    Unpinned,
    Full,
}

fn toggled(mut ids: Vec<String>, id: &str) -> (Vec<String>, PinChange) {
    if let Some(at) = ids.iter().position(|p| p == id) {
        ids.remove(at);
        (ids, PinChange::Unpinned)
    } else if ids.len() >= bar_slot::MAX_PINNED {
        (ids, PinChange::Full)
    } else {
        ids.push(id.to_string());
        (ids, PinChange::Pinned)
    }
}

/// Writes the config; the caller loads it and refreshes the bar. Starts from
/// the file, one toggle at a time, so two in a row both land.
pub fn toggle_pin(id: &str) -> Result<PinChange, String> {
    static TOGGLING: Mutex<()> = Mutex::new(());
    let _toggling = TOGGLING.lock().unwrap_or_else(|p| p.into_inner());
    let saved = pinned_in(&theme::read_entries());
    let (ids, change) = toggled(config_list::parse(&saved), id);
    if change != PinChange::Full {
        config::set_config(vec![ConfigUpdate {
            key: bar_slot::PINNED_APPS_KEY.into(),
            value: config_list::render(&ids),
        }])?;
    }
    Ok(change)
}

/// The settings screen's unsaved pick, until the config is read again.
static PICKED: Mutex<Option<BarSlot>> = Mutex::new(None);

fn picked() -> std::sync::MutexGuard<'static, Option<BarSlot>> {
    PICKED.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn session_slot() -> Option<BarSlot> {
    *picked()
}

pub fn pick_for_session(slot: BarSlot) {
    *picked() = Some(slot);
}

#[derive(Default)]
pub struct Bar {
    slot: BarSlot,
    pub(crate) running: RunningApps,
    todo: Option<TodoLine>,
    script: Option<SharedString>,
}

/// A task and the day's count, or the note alone.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TodoLine {
    task: Option<String>,
    note: String,
}

fn todo_line(today: &Today, pick: usize) -> TodoLine {
    let count = format!("{}/{}", today.done, today.total);
    let (task, note) = match (today.total, today.open.len()) {
        (0, _) => (None, TODO_EMPTY.into()),
        (_, 0) => (None, format!("{count} {TODO_ALL_DONE}")),
        (_, open) => (Some(today.open[pick % open].clone()), count),
    };
    TodoLine { task, note }
}

/// Kept for the process: the window is rebuilt on every summon.
struct Script {
    /// The command and what it gave.
    last: Option<(String, Result<String, String>)>,
    running: bool,
}

static SCRIPT: Mutex<Script> = Mutex::new(Script {
    last: None,
    running: false,
});

fn script() -> std::sync::MutexGuard<'static, Script> {
    SCRIPT.lock().unwrap_or_else(|p| p.into_inner())
}

/// What `command` last printed or why it failed; None until it has run.
pub fn script_result(command: &str) -> Option<Result<String, String>> {
    script()
        .last
        .as_ref()
        .filter(|(ran, _)| ran == command)
        .map(|(_, result)| result.clone())
}

fn script_command() -> String {
    theme::entries()
        .get(bar_slot::SCRIPT_KEY)
        .map(|c| c.trim().to_string())
        .unwrap_or_default()
}

fn script_text(command: &str) -> Option<SharedString> {
    script_result(command)
        .and_then(Result::ok)
        .filter(|line| !line.is_empty())
        .map(SharedString::from)
}

impl Bar {
    /// The config was read again: it wins over the session's pick.
    pub fn reload(&mut self, cx: &mut Context<Launcher>) {
        *picked() = None;
        self.refresh(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Launcher>) {
        self.slot = session_slot().unwrap_or_else(|| BarSlot::from_entries(&theme::entries()));
        self.refresh_pins(cx);
        match self.slot {
            BarSlot::Running => self.running.refresh(cx),
            BarSlot::Todo => {
                let pick = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.subsec_nanos() as usize);
                bg::fetch(
                    cx,
                    || store::todo_list().unwrap_or_default(),
                    move |this, tasks, _| {
                        this.bar.todo = Some(todo_line(&Today::of(tasks), pick));
                    },
                );
            }
            BarSlot::Script => {
                let command = script_command();
                self.script = script_text(&command);
                self.run_script(command, cx);
            }
            BarSlot::Pinned | BarSlot::None => {}
        }
    }

    pub fn refresh_pins(&mut self, cx: &mut Context<Launcher>) {
        let ids = bar_slot::pinned_ids(&pinned_value());
        if ids.is_empty() {
            pins().clear();
            return;
        }
        bg::fetch(
            cx,
            move || resolve(ids),
            |_, found, _| {
                let saved = bar_slot::pinned_ids(&pinned_value());
                if found.iter().map(|pin| &pin.id).eq(&saved) {
                    *pins() = found;
                }
            },
        );
    }

    /// One run at a time. The answer goes to the launcher on screen by then,
    /// not the one that asked: the window is rebuilt on every summon.
    fn run_script(&mut self, command: String, cx: &mut Context<Launcher>) {
        if command.is_empty() {
            return;
        }
        if std::mem::replace(&mut script().running, true) {
            return;
        }
        cx.spawn(async move |_, cx| {
            bg::blocking(move || {
                let result = bar_slot::script_line(&command);
                *script() = Script {
                    last: Some((command, result)),
                    running: false,
                };
            })
            .get()
            .await;
            cx.update(|cx| {
                crate::with_launcher(cx, |this, cx| {
                    this.bar.script_finished(cx);
                    cx.notify();
                })
            });
        })
        .detach();
    }

    /// Runs again when the command changed while the last one ran.
    fn script_finished(&mut self, cx: &mut Context<Launcher>) {
        if self.slot != BarSlot::Script {
            return;
        }
        let command = script_command();
        self.script = script_text(&command);
        if script_result(&command).is_none() {
            self.run_script(command, cx);
        }
    }

    /// Alt+digit.
    pub fn activate_key(&self, key: u8, shell: &Shell, cx: &mut Context<Launcher>) -> bool {
        match self.slot {
            BarSlot::Running => self.running.activate_key(key, shell, cx),
            BarSlot::Pinned => {
                let count = shown(&pins()).count();
                running::key_position(count, key).is_some_and(|at| open_pin(at, shell))
            }
            _ => false,
        }
    }

    pub fn render(
        &self,
        icons: &gpui::Entity<IconStore>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Option<Div> {
        match self.slot {
            BarSlot::Running => self.running.render(icons, th, cx),
            BarSlot::Todo => self.todo.as_ref().map(|line| {
                let text = text_slot(th)
                    .children(line.task.clone().map(|task| {
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(th.text_secondary)
                            .child(SharedString::from(task))
                    }))
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_family(th.mono_family.clone())
                            .child(SharedString::from(line.note.clone())),
                    );
                div().flex_shrink_0().child(slide_in(
                    text.id("bar-todo")
                        .cursor_pointer()
                        .hover(|s| s.opacity(1.0))
                        .on_click(cx.listener(|this, _, _, cx| this.open_todo(cx))),
                ))
            }),
            BarSlot::Script => self.script.clone().map(|line| {
                div().flex_shrink_0().child(slide_in(
                    text_slot(th)
                        .id("bar-script")
                        .child(div().min_w_0().truncate().child(line)),
                ))
            }),
            BarSlot::Pinned => {
                let tiles = shown(&pins())
                    .map(|pin| {
                        Some(running::Tile {
                            id: pin.id.clone(),
                            path: pin.path.clone(),
                        })
                    })
                    .collect();
                running::strip(tiles, icons, th, cx, |this, at, _| {
                    open_pin(at, &this.shell);
                })
            }
            BarSlot::None => None,
        }
    }
}

fn resolve(ids: Vec<String>) -> Vec<Pin> {
    crate::state().with_engine(|engine| {
        ids.into_iter()
            .map(|id| {
                let (title, path) = engine.title_and_path(&id).unzip();
                Pin {
                    title: title.map(str::to_string),
                    path: path.unwrap_or_default().to_string(),
                    id,
                }
            })
            .collect()
    })
}

/// The pins the bar draws.
fn shown(pins: &[Pin]) -> impl Iterator<Item = &Pin> {
    pins.iter().filter(|pin| pin.title.is_some())
}

/// Opens through the row path, which focuses a running instance first.
fn open_pin(at: usize, shell: &Shell) -> bool {
    let Some(pin) = shown(&pins()).nth(at).cloned() else {
        return false;
    };
    match launch::open_path(shell, pin.path, Some(APP_KIND), Some(&pin.id)) {
        Ok(()) => {
            search::record_usage(crate::state(), &pin.id, APP_USAGE);
        }
        Err(err) => eprintln!("[pinned] open failed: {err}"),
    }
    true
}

fn text_slot(th: &Theme) -> Div {
    div()
        .max_w(px(running::FULL_WIDTH))
        .flex()
        .items_center()
        .gap(px(TEXT_GAP))
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_muted)
}

/// The strip's entrance.
fn slide_in(el: gpui::Stateful<Div>) -> impl IntoElement {
    el.relative().with_animation(
        "bar-slot",
        Animation::new(motion::dur(motion::SLIDE_MS)),
        |el, progress| {
            let t = motion::curve(progress);
            el.opacity(t)
                .left(px(motion::rise(0.0, motion::STRIP_SHIFT, t)))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today(done: usize, open: &[&str]) -> Today {
        Today {
            done,
            total: done + open.len(),
            open: open.iter().map(|name| name.to_string()).collect(),
        }
    }

    #[test]
    fn an_empty_day_asks_for_a_plan() {
        let line = todo_line(&today(0, &[]), 7);
        assert_eq!(line.task, None);
        assert_eq!(line.note, TODO_EMPTY);
    }

    #[test]
    fn a_finished_day_shows_only_the_count() {
        let line = todo_line(&today(3, &[]), 7);
        assert_eq!(line.task, None);
        assert_eq!(line.note, "3/3 done");
    }

    #[test]
    fn an_open_task_comes_with_the_count() {
        let day = today(1, &["write", "ship"]);
        assert_eq!(todo_line(&day, 0).task.as_deref(), Some("write"));
        assert_eq!(todo_line(&day, 5).task.as_deref(), Some("ship"));
        assert_eq!(todo_line(&day, 5).note, "1/3");
    }

    #[test]
    fn a_pin_toggles_and_the_tenth_is_refused() {
        let ids = |n: usize| (0..n).map(|i| i.to_string()).collect::<Vec<_>>();
        assert_eq!(
            toggled(ids(2), "x"),
            (vec!["0".into(), "1".into(), "x".into()], PinChange::Pinned)
        );
        assert_eq!(
            toggled(ids(2), "0"),
            (vec!["1".into()], PinChange::Unpinned)
        );
        assert_eq!(toggled(ids(9), "x"), (ids(9), PinChange::Full));
        assert_eq!(toggled(ids(9), "8").1, PinChange::Unpinned);
    }

    #[test]
    fn a_result_belongs_to_the_command_that_gave_it() {
        script().last = Some(("date".into(), Ok("09:41".into())));
        assert_eq!(script_result("date"), Some(Ok("09:41".into())));
        assert_eq!(script_result("uptime"), None);
        assert_eq!(script_text("date").as_deref(), Some("09:41"));
    }
}
