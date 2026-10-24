//! Quick Actions: the live controls the shared catalog gives some rows
//! (Bluetooth, Wi-Fi, ...), under the preview's header. The webview's
//! `components/qactions.js`: the switch or Ctrl+O flips the first toggle, a
//! device row connects or disconnects it, the outcome goes to the banner,
//! and the state is read again after every apply.

use gpui::{Context, Div, ElementId, FontWeight, SharedString, div, prelude::*, px};
use linows_backend::look_qactions::{ActionDescriptor, ControlKind};
use linows_backend::qactions::{
    self, ActionIntent, ActionOutcome, ActionState, InfoValue, ListItem, QuickActionStatus,
};

use crate::bg;
use crate::controls;
use crate::launchpad::{Notice, Tone};
use crate::preview::{self, Preview};
use crate::rows::Row;
use crate::shortcuts;
use crate::theme::Theme;

/// Matches the backend's device timeout, so "Connecting..." stays up for the
/// whole wait and the outcome takes over when it lands.
const DEVICE_PENDING_SECONDS: f32 = 6.0;
const PERMISSION_SECONDS: f32 = 2.2;
const SECTION_GAP: f32 = 6.0;
const SECTION_MARGIN: f32 = 12.0;
const ROW_PADDING_X: f32 = 10.0;
const ROW_PADDING_Y: f32 = 8.0;
const CONTROL_GAP: f32 = 8.0;
const DEVICE_PADDING_X: f32 = 6.0;
const DEVICE_PADDING_Y: f32 = 4.0;
const DEVICE_GAP: f32 = 8.0;
const DOT: f32 = 7.0;
const DOT_BORDER: f32 = 1.5;
const INERT_OPACITY: f32 = 0.5;
const NONE_CONNECTED: &str = "None connected";

/// The actions of the row on screen.
pub struct QuickActions {
    row_id: String,
    actions: Vec<Action>,
    /// One apply at a time; every control goes inert while it runs.
    busy: bool,
}

struct Action {
    descriptor: ActionDescriptor,
    status: Option<QuickActionStatus>,
    /// The switch as drawn: flipped at the press, confirmed by the re-read.
    on: Option<bool>,
}

impl Action {
    fn available(&self) -> bool {
        self.status
            .as_ref()
            .is_some_and(|s| !matches!(s.state, ActionState::Unavailable { .. }))
    }
}

impl Preview {
    /// The catalog's actions for `row`, read fresh. Kept as they are while
    /// the same row stays selected.
    pub(crate) fn sync_actions(&mut self, row: &Row, cx: &mut Context<Self>) {
        if self.actions.as_ref().is_some_and(|a| a.row_id == row.id) {
            return;
        }
        let descriptors = qactions::quick_actions(&row.id, &row.kind);
        if descriptors.is_empty() {
            self.actions = None;
            return;
        }
        self.actions = Some(QuickActions {
            row_id: row.id.clone(),
            actions: descriptors
                .into_iter()
                .map(|descriptor| Action {
                    descriptor,
                    status: None,
                    on: None,
                })
                .collect(),
            busy: false,
        });
        self.read_actions(cx);
    }

    /// Ctrl+O: flip the first toggle. False when the row has none.
    pub fn toggle_primary(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.actions.as_ref().and_then(|a| {
            a.actions
                .iter()
                .position(|it| it.descriptor.control == ControlKind::Toggle && it.available())
        }) else {
            return false;
        };
        self.run_action(index, cx);
        true
    }

    fn read_actions(&mut self, cx: &mut Context<Self>) {
        let Some(actions) = &self.actions else {
            return;
        };
        for index in 0..actions.actions.len() {
            self.read_action(index, cx);
        }
    }

    fn read_action(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(actions) = &self.actions else {
            return;
        };
        let row_id = actions.row_id.clone();
        let descriptor = &actions.actions[index].descriptor;
        let id = descriptor.action_id.clone();
        let keys: Vec<String> = descriptor
            .info
            .iter()
            .map(|f| f.value_key.clone())
            .collect();
        bg::fetch(
            cx,
            move || qactions::quick_action_state(&id, &keys),
            move |this, status, _| {
                if let Some(action) = this.action_mut(&row_id, index) {
                    action.on = match status.state {
                        ActionState::On => Some(true),
                        ActionState::Off => Some(false),
                        _ => None,
                    };
                    action.status = Some(status);
                }
            },
        );
    }

    fn action_mut(&mut self, row_id: &str, index: usize) -> Option<&mut Action> {
        self.actions
            .as_mut()
            .filter(|a| a.row_id == row_id)
            .and_then(|a| a.actions.get_mut(index))
    }

    /// The switch or the button. A switch press asks for the opposite of
    /// what is drawn, so a stale panel still does what the user sees.
    fn run_action(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(actions) = self.actions.as_mut().filter(|a| !a.busy) else {
            return;
        };
        let action = &mut actions.actions[index];
        if !action.available() {
            return;
        }
        let intent = match (action.descriptor.control, action.on) {
            (ControlKind::Button, _) => ActionIntent::Run,
            (ControlKind::Toggle, Some(on)) => {
                action.on = Some(!on);
                ActionIntent::SetOn(!on)
            }
            (ControlKind::Toggle, None) => ActionIntent::Toggle,
        };
        actions.busy = true;
        let id = action.descriptor.action_id.clone();
        self.apply(index, move || qactions::quick_action_apply(&id, intent), cx);
        cx.notify();
    }

    /// A device row: connect it if it is off, disconnect it if it is on.
    fn run_item(&mut self, index: usize, item: &ListItem, cx: &mut Context<Self>) {
        let Some(item_id) = item.id.clone() else {
            return;
        };
        let Some(actions) = self.actions.as_mut().filter(|a| !a.busy) else {
            return;
        };
        actions.busy = true;
        let verb = if item.on == Some(true) {
            "Disconnecting"
        } else {
            "Connecting to"
        };
        cx.emit(Notice {
            text: format!("{verb} {}\u{2026}", item.label),
            tone: Tone::Info,
            seconds: DEVICE_PENDING_SECONDS,
            copy: None,
        });
        let id = actions.actions[index].descriptor.action_id.clone();
        self.apply(
            index,
            move || qactions::quick_action_apply_item(&id, &item_id, ActionIntent::Toggle),
            cx,
        );
        cx.notify();
    }

    /// Apply off the UI thread, say how it went, read the state again.
    fn apply(
        &mut self,
        index: usize,
        work: impl FnOnce() -> ActionOutcome + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(actions) = &self.actions else {
            return;
        };
        let row_id = actions.row_id.clone();
        let title = actions.actions[index].descriptor.title.clone();
        bg::fetch(cx, work, move |this, outcome, cx| {
            let Some(actions) = this.actions.as_mut().filter(|a| a.row_id == row_id) else {
                return;
            };
            actions.busy = false;
            cx.emit(notice(&title, outcome));
            this.read_action(index, cx);
        });
    }

    /// The section under the header, when the row has actions.
    pub(crate) fn actions_section(&self, th: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let actions = self.actions.as_ref()?;
        let busy = actions.busy;
        let mut section = div()
            .flex_shrink_0()
            .mb(px(SECTION_MARGIN))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP));
        for (index, action) in actions.actions.iter().enumerate() {
            section = section.child(self.control_row(index, action, busy, th, cx));
            if action.descriptor.info.is_empty() {
                continue;
            }
            let info = action.status.as_ref().map(|s| &s.info);
            let mut list = preview::info_list(th);
            for field in &action.descriptor.info {
                let value = info.and_then(|i| i.get(&field.value_key));
                let text = match value {
                    Some(InfoValue::List { items }) => connected_count(items),
                    Some(InfoValue::Text { text: v } | InfoValue::Unavailable { reason: v }) => {
                        v.clone()
                    }
                    None => String::new(),
                };
                list = list.child(preview::info_row(field.label.clone(), text, th));
                if let Some(InfoValue::List { items }) = value {
                    for (i, item) in items.iter().enumerate() {
                        list = list.child(self.device_row(index, i, item, busy, th, cx));
                    }
                }
            }
            section = section.child(list);
        }
        Some(section)
    }

    /// Title left; the switch and its key, the button, or why it cannot act,
    /// right.
    fn control_row(
        &self,
        index: usize,
        action: &Action,
        busy: bool,
        th: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let control = match &action.status.as_ref().map(|s| &s.state) {
            Some(ActionState::Unavailable { reason }) => div()
                .text_size(px(th.font_size - 3.0))
                .text_color(th.text_muted)
                .child(SharedString::from(reason.clone()))
                .into_any_element(),
            _ => {
                let live = action.available() && !busy;
                let press = cx.listener(move |this, _, _, cx| this.run_action(index, cx));
                let control = match action.descriptor.control {
                    ControlKind::Toggle => {
                        controls::toggle(("qaction-toggle", index), action.on == Some(true), th)
                    }
                    ControlKind::Button => controls::button(
                        ("qaction-button", index),
                        action.descriptor.title.clone(),
                        th,
                    ),
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(CONTROL_GAP))
                    .child(
                        control
                            .when(live, |el| el.on_click(press))
                            .when(!live, |el| el.opacity(INERT_OPACITY)),
                    )
                    .when(
                        index == 0 && action.descriptor.control == ControlKind::Toggle,
                        |el| {
                            el.children(shortcuts::entry(shortcuts::QUICK_TOGGLE).map(|e| {
                                div()
                                    .text_size(px(th.font_size - 3.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(th.text_muted)
                                    .child(SharedString::from(e.keys.clone()))
                            }))
                        },
                    )
                    .into_any_element()
            }
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(ROW_PADDING_X))
            .px(px(ROW_PADDING_X))
            .py(px(ROW_PADDING_Y))
            .rounded(px(th.control_radius()))
            .bg(th.control_fill)
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(SharedString::from(action.descriptor.title.clone())),
            )
            .child(control)
    }

    /// A device: a dot, hollow when paired and filled when connected, then
    /// its name. Clickable when the adapter gave it an id.
    fn device_row(
        &self,
        index: usize,
        position: usize,
        item: &ListItem,
        busy: bool,
        th: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let connected = item.on == Some(true);
        let clickable = item.id.is_some();
        let clicked = item.clone();
        div()
            .id(ElementId::NamedInteger(
                format!("qaction-device-{index}").into(),
                position as u64,
            ))
            .flex()
            .items_center()
            .gap(px(DEVICE_GAP))
            .min_w_0()
            .px(px(DEVICE_PADDING_X))
            .py(px(DEVICE_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .when(clickable && !busy, |el| {
                el.cursor_pointer()
                    .hover(|s| s.bg(th.control_fill))
                    .on_click(cx.listener(move |this, _, _, cx| this.run_item(index, &clicked, cx)))
            })
            .when(clickable && busy, |el| el.opacity(INERT_OPACITY))
            .child(
                div()
                    .size(px(DOT))
                    .flex_shrink_0()
                    .rounded_full()
                    .border(px(DOT_BORDER))
                    .border_color(if connected { th.accent } else { th.text_muted })
                    .when(connected, |el| el.bg(th.accent)),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_secondary)
                    .child(SharedString::from(item.label.clone())),
            )
    }
}

fn connected_count(items: &[ListItem]) -> String {
    match items.iter().filter(|it| it.on == Some(true)).count() {
        0 => NONE_CONNECTED.to_string(),
        n => format!("{n} connected"),
    }
}

/// The banner line for an outcome, the webview's `showOutcome`.
fn notice(title: &str, outcome: ActionOutcome) -> Notice {
    match outcome {
        ActionOutcome::Ok { banner } => Notice {
            text: banner.unwrap_or_else(|| format!("{title} done")),
            tone: Tone::Success,
            seconds: crate::banner::MEDIUM,
            copy: None,
        },
        ActionOutcome::NeedsPermission { message } => Notice {
            text: message,
            tone: Tone::Info,
            seconds: PERMISSION_SECONDS,
            copy: None,
        },
        ActionOutcome::Failed { message } => Notice {
            text: if message.is_empty() {
                format!("{title} failed")
            } else {
                message
            },
            tone: Tone::Error,
            seconds: crate::banner::LONG,
            copy: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(on: Option<bool>) -> ListItem {
        ListItem {
            id: None,
            label: String::new(),
            on,
        }
    }

    #[test]
    fn the_device_header_counts_the_connected() {
        assert_eq!(
            connected_count(&[item(Some(false)), item(None)]),
            NONE_CONNECTED
        );
        assert_eq!(
            connected_count(&[item(Some(true)), item(Some(true)), item(Some(false))]),
            "2 connected"
        );
    }

    #[test]
    fn an_outcome_falls_back_to_the_action_title() {
        let ok = notice("Bluetooth", ActionOutcome::Ok { banner: None });
        assert_eq!(ok.text, "Bluetooth done");
        let failed = notice(
            "Bluetooth",
            ActionOutcome::Failed {
                message: String::new(),
            },
        );
        assert_eq!(failed.text, "Bluetooth failed");
        assert!(failed.tone == Tone::Error);
    }
}
