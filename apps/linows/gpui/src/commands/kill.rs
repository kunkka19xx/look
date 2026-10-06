//! `/kill`: the running apps on entry, a fuzzy search over every process as
//! it is typed (ports and pids included), and a Y or N question under the
//! list before anything dies.

use std::time::Duration;

use gpui::{Context, Div, Entity, FontWeight, Stateful, Task, div, img, prelude::*, px};
use linows_backend::process::{self, KillTarget};

use super::{Commands, KeyOutcome};
use crate::banner;
use crate::bg;
use crate::icons::{IconRequest, IconStore};
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::rows;
use crate::theme::Theme;

pub const ID: &str = "kill";
const PLACEHOLDER: &str = "Type app name, port, or PID";
const ROW_PADDING_X: f32 = 10.0;
const ROW_PADDING_Y: f32 = 7.0;
const ROW_GAP: f32 = 8.0;
const ROW_INNER_GAP: f32 = 4.0;
const ROW_ICON: f32 = 22.0;
const ICON_RADIUS: f32 = 4.0;
const CONFIRM_ICON: f32 = 28.0;
const CONFIRM_PADDING_X: f32 = 14.0;
const CONFIRM_PADDING_Y: f32 = 10.0;
const CONFIRM_GAP: f32 = 10.0;
const CONFIRM_BUTTON_GAP: f32 = 12.0;
const CONFIRM_YES_PADDING_X: f32 = 12.0;
const CONFIRM_YES_PADDING_Y: f32 = 4.0;
/// A keystroke waits this long before `/proc` is walked.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(140);
/// The kill banner shows before the list reloads without the row.
const RELOAD_DELAY: Duration = Duration::from_millis(300);
const LOADING: &str = "Loading...";
const SEARCHING: &str = "Searching...";
const NO_PROCESSES: &str = "No running processes";
const NO_MATCH: &str = "No matching processes";
const ENTER_MARK: &str = "\u{2192} Enter";
const YES: &str = "Y / Yes";
const NO: &str = "N / No";

#[derive(Default)]
pub struct Kill {
    /// The apps, listed on entry and after a kill.
    base: Vec<KillTarget>,
    base_loaded: bool,
    rows: Vec<KillTarget>,
    selected: usize,
    /// The pid awaiting Y.
    confirm: Option<u32>,
    /// Per keystroke; a search answering for an older one is dropped.
    generation: u64,
    /// A backend search is scheduled or in flight: the local name filter
    /// may miss the port and pid matches it owns.
    pending: bool,
    _search: Option<Task<()>>,
}

impl Kill {
    pub fn enter(&mut self, cx: &mut Context<Launcher>) {
        *self = Self::default();
        self.load(cx);
    }

    /// Escape: drop the question. True when there was one.
    pub fn dismiss(&mut self, cx: &mut Context<Launcher>) -> bool {
        let had = self.confirm.take().is_some();
        if had {
            cx.notify();
        }
        had
    }

    fn load(&mut self, cx: &mut Context<Launcher>) {
        bg::fetch(
            cx,
            || {
                process::list_processes()
                    .into_iter()
                    .map(|a| KillTarget {
                        name: a.name,
                        pid: a.pid,
                        is_app: true,
                        desktop_id: a.desktop_id,
                        exec: a.exec,
                        ports: Vec::new(),
                    })
                    .collect::<Vec<_>>()
            },
            |this, apps, cx| {
                let kill = &mut this.commands.kill;
                kill.base = apps;
                kill.base_loaded = true;
                kill.confirm = None;
                let query = this.commands.typed(cx);
                this.commands.kill.filter(&query, cx);
            },
        );
    }

    /// An instant provisional list from the loaded apps, then the backend's
    /// fuzzy result (apps first, then processes with port and pid matches).
    pub fn filter(&mut self, query: &str, cx: &mut Context<Launcher>) {
        self.generation += 1;
        self.confirm = None;
        if query.is_empty() {
            self._search = None;
            self.pending = false;
            self.rows = self.base.clone();
        } else {
            let needle = query.to_lowercase();
            self.rows = self
                .base
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&needle))
                .cloned()
                .collect();
            self.pending = true;
            let generation = self.generation;
            let query = query.to_string();
            self._search = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SEARCH_DEBOUNCE).await;
                let targets = cx
                    .background_executor()
                    .spawn(async move { process::search_kill_targets(&query) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let kill = &mut this.commands.kill;
                    if kill.generation != generation {
                        return;
                    }
                    kill.pending = false;
                    kill.rows = targets;
                    kill.selected = 0;
                    kill.confirm = None;
                    cx.notify();
                });
            }));
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    pub fn key(&mut self, key: &str, cx: &mut Context<Launcher>) -> KeyOutcome {
        if let Some(pid) = self.confirm {
            match key {
                "y" => {
                    self.confirm = None;
                    self.kill_pid(pid, cx);
                }
                "n" => {
                    self.confirm = None;
                    cx.notify();
                }
                _ => {}
            }
            return KeyOutcome::Consumed;
        }
        match key {
            "down" if !self.rows.is_empty() => {
                self.selected = (self.selected + 1).min(self.rows.len() - 1);
            }
            "up" if !self.rows.is_empty() => {
                self.selected = self.selected.saturating_sub(1);
            }
            "enter" => {
                self.confirm = self.rows.get(self.selected).map(|p| p.pid);
            }
            _ => return KeyOutcome::Pass,
        }
        cx.notify();
        KeyOutcome::Consumed
    }

    fn kill_pid(&mut self, pid: u32, cx: &mut Context<Launcher>) {
        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move { process::kill_process(pid) })
                .await;
            let _ = this.update(cx, |this, cx| match outcome {
                Ok(message) => this.banner.show(message, Tone::Success, banner::MEDIUM, cx),
                Err(err) => this.banner.show(err, Tone::Error, banner::LONG, cx),
            });
            // The banner reads before the list drops the row.
            cx.background_executor().timer(RELOAD_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if this.commands.entry().id == ID {
                    this.commands.kill.load(cx);
                }
            });
        })
        .detach();
    }
}

pub fn panel(
    frame: &Commands,
    icons: &Entity<IconStore>,
    th: &Theme,
    cx: &mut Context<Launcher>,
) -> Div {
    let kill = &frame.kill;
    let typed = !frame.typed(cx).is_empty();
    let feedback = if !kill.rows.is_empty() {
        None
    } else if !typed {
        Some(if kill.base_loaded {
            NO_PROCESSES
        } else {
            LOADING
        })
    } else if kill.pending {
        Some(SEARCHING)
    } else {
        Some(NO_MATCH)
    };
    let picture = |target: &KillTarget, size: f32, cx: &mut Context<Launcher>| {
        let desktop_id = target.desktop_id.clone()?;
        let image = icons.update(cx, |store, cx| {
            store.get(
                IconRequest {
                    kind: "app".into(),
                    path: target.exec.clone().unwrap_or_default(),
                    id: Some(desktop_id),
                },
                cx,
            )
        })?;
        Some(img(image).size(px(size)).rounded(px(ICON_RADIUS)))
    };
    let rows: Vec<Stateful<Div>> = kill
        .rows
        .iter()
        .enumerate()
        .map(|(i, target)| {
            let active = i == kill.selected;
            let icon = picture(target, ROW_ICON, cx);
            let pid = target.pid;
            div()
                .id(("proc", i))
                .px(px(ROW_PADDING_X))
                .py(px(ROW_PADDING_Y))
                .rounded(px(th.chip_radius()))
                .when(active, |el| el.bg(th.selection_fill))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.commands.kill.selected = i;
                    this.commands.kill.confirm = Some(pid);
                    cx.notify();
                }))
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .children(icon)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(target.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(ROW_INNER_GAP))
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_secondary)
                        .child(rows::process_pid_label(target.pid, &target.ports))
                        .when(active, |el| {
                            el.child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(th.accent)
                                    .child(ENTER_MARK),
                            )
                        }),
                )
        })
        .collect();
    let confirm = kill
        .confirm
        .and_then(|pid| kill.rows.iter().find(|p| p.pid == pid))
        .map(|target| {
            let icon = picture(target, CONFIRM_ICON, cx);
            div()
                .flex_shrink_0()
                .px(px(CONFIRM_PADDING_X))
                .py(px(CONFIRM_PADDING_Y))
                .border_t(px(1.0))
                .border_color(th.border)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(CONFIRM_GAP))
                        .children(icon)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("Kill {}?", target.name)),
                                )
                                .child(
                                    div()
                                        .font_family(th.mono_family.clone())
                                        .text_size(px(th.font_size - 2.0))
                                        .text_color(th.text_secondary)
                                        .child(rows::process_pid_label(target.pid, &target.ports)),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(CONFIRM_BUTTON_GAP))
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size - 1.0))
                        .child(
                            div()
                                .px(px(CONFIRM_YES_PADDING_X))
                                .py(px(CONFIRM_YES_PADDING_Y))
                                .rounded(px(th.chip_radius()))
                                .bg(th.danger)
                                .text_color(th.on_accent)
                                .font_weight(FontWeight::BOLD)
                                .child(YES),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(th.text_secondary)
                                .child(NO),
                        ),
                )
        });
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(frame.input_bar(PLACEHOLDER, th))
        .child(
            frame
                .content()
                .when_some(feedback, |el, text| {
                    el.child(frame.feedback(th).child(text))
                })
                .child(
                    div()
                        .id("kill-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .children(rows),
                ),
        )
        .children(confirm)
}
