//! `/kill`: the running apps on entry, a fuzzy search over every process as
//! it is typed (ports and pids included), and a Y or N question under the
//! list before anything dies.
//!
//! `/proc` is walked once on entry and again after a kill; each keystroke
//! ranks that snapshot in memory on the same frame, so the list never
//! re-sorts behind the typing.

use std::time::Duration;

use gpui::{Context, Div, Entity, FontWeight, Stateful, div, img, prelude::*, px};
use linows_backend::process::{self, KillSnapshot, KillTarget};

use super::{Commands, KeyOutcome};
use crate::banner;
use crate::bg;
use crate::confirm::{NO_LABEL, YES_LABEL};
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
/// The kill banner shows before the list reloads without the row.
const RELOAD_DELAY: Duration = Duration::from_millis(300);
const LOADING: &str = "Loading...";
const NO_PROCESSES: &str = "No running processes";
const NO_MATCH: &str = "No matching processes";
const ENTER_MARK: &str = "\u{2192} Enter";

#[derive(Default)]
pub struct Kill {
    /// Apps and processes as of entry or the last kill; `None` while the
    /// walk is out.
    snapshot: Option<KillSnapshot>,
    rows: Vec<KillTarget>,
    selected: usize,
    /// The pid awaiting Y.
    confirm: Option<u32>,
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
        bg::fetch(cx, KillSnapshot::take, |this, snapshot, cx| {
            this.commands.kill.snapshot = Some(snapshot);
            let query = this.commands.typed(cx);
            this.commands.kill.filter(&query, cx);
        });
    }

    /// The snapshot ranked for `query`: apps first, then processes with
    /// their port and pid matches. In memory, so it lands on this frame.
    pub fn filter(&mut self, query: &str, cx: &mut Context<Launcher>) {
        self.confirm = None;
        self.rows = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.rank(query))
            .unwrap_or_default();
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
            let outcome = bg::blocking(move || process::kill_process(pid)).get().await;
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
    } else if kill.snapshot.is_none() {
        Some(LOADING)
    } else if typed {
        Some(NO_MATCH)
    } else {
        Some(NO_PROCESSES)
    };
    let picture = |target: &KillTarget, size: f32, cx: &mut Context<Launcher>| {
        let desktop_id = target.desktop_id.clone()?;
        let image = icons.update(cx, |store, cx| {
            store.get(
                IconRequest {
                    kind: "app".into(),
                    path: target.exec.clone().unwrap_or_default(),
                    id: Some(desktop_id),
                    size,
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
                                .child(YES_LABEL),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(th.text_secondary)
                                .child(NO_LABEL),
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
