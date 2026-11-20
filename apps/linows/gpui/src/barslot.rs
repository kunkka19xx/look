//! The slot at the right end of the top bar, holding what `bar_slot` says.
//! Read on each open; hidden in the compact layout and while translating.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{Animation, AnimationExt, Context, Div, SharedString, div, prelude::*, px};
use linows_backend::look_engine::bar_slot::{self, BarSlot};
use linows_backend::todo as store;

use crate::Shell;
use crate::bg;
use crate::icons::IconStore;
use crate::launcher::Launcher;
use crate::motion;
use crate::running::{self, RunningApps};
use crate::theme::{self, Theme};
use crate::todo::Today;

const TODO_EMPTY: &str = "Plan today";
const TODO_ALL_DONE: &str = "done";
const TEXT_GAP: f32 = 8.0;

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

fn script_text(command: &str) -> Option<SharedString> {
    script_result(command)
        .and_then(Result::ok)
        .filter(|line| !line.is_empty())
        .map(SharedString::from)
}

impl Bar {
    pub fn refresh(&mut self, cx: &mut Context<Launcher>) {
        let entries = theme::entries();
        self.slot = BarSlot::from_entries(&entries);
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
                let command = entries
                    .get(bar_slot::SCRIPT_KEY)
                    .map(|c| c.trim().to_string())
                    .unwrap_or_default();
                self.script = script_text(&command);
                self.run_script(command, cx);
            }
            BarSlot::Pinned | BarSlot::None => {}
        }
    }

    /// One run at a time.
    fn run_script(&mut self, command: String, cx: &mut Context<Launcher>) {
        if command.is_empty() {
            return;
        }
        if std::mem::replace(&mut script().running, true) {
            return;
        }
        bg::fetch(
            cx,
            move || {
                let result = bar_slot::script_line(&command);
                *script() = Script {
                    last: Some((command.clone(), result)),
                    running: false,
                };
                command
            },
            |this, command, _| this.bar.script = script_text(&command),
        );
    }

    /// Alt+digit.
    pub fn activate_key(&self, key: u8, shell: &Shell, cx: &mut Context<Launcher>) -> bool {
        self.slot == BarSlot::Running && self.running.activate_key(key, shell, cx)
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
            BarSlot::Pinned | BarSlot::None => None,
        }
    }
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
    fn a_result_belongs_to_the_command_that_gave_it() {
        script().last = Some(("date".into(), Ok("09:41".into())));
        assert_eq!(script_result("date"), Some(Ok("09:41".into())));
        assert_eq!(script_result("uptime"), None);
        assert_eq!(script_text("date").as_deref(), Some("09:41"));
    }
}
