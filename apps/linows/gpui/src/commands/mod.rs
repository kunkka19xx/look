//! The command screen (`/`): a sidebar of the catalog's commands beside the
//! active panel, in one framed card that takes the window while it is up.
//! Mirrors `screens/commands/index.js`: this module is the frame, and each
//! command is a file of its own beside it; the ones not ported yet say so
//! in place of their body.

mod calc;
mod kill;
mod pomo;
mod shell;
mod sys;
mod todo;

use gpui::{AnyElement, Context, Div, Entity, FontWeight, deferred, div, prelude::*, px, svg};

use crate::icons::IconStore;
use crate::launcher::Launcher;
use crate::modes::{COMMAND_ENTRIES, CommandEntry};
use crate::search::{SearchInput, search_field};
use crate::theme::Theme;

const SIDEBAR_W: f32 = 180.0;
const SIDEBAR_PADDING: f32 = 5.0;
const SIDEBAR_GAP: f32 = 2.0;
const SIDEBAR_ROW_PADDING_X: f32 = 8.0;
const SIDEBAR_ROW_PADDING_Y: f32 = 6.0;
const SIDEBAR_ROW_GAP: f32 = 8.0;
const SIDEBAR_ICON_BOX: f32 = 22.0;
const SIDEBAR_ICON: f32 = 16.0;
const DIVIDER_MARGIN: f32 = 2.0;
/// The rows' two lines sit tight, as the CSS sets them.
const SIDEBAR_LINE_HEIGHT: f32 = 1.2;
/// The input bar's inset equals the content inset below it, so a command's
/// bar and its output share edges.
const BAR_MARGIN_X: f32 = 18.0;
const BAR_MARGIN_TOP: f32 = 8.0;
const BAR_PADDING_X: f32 = 12.0;
const BAR_PADDING_Y: f32 = 10.0;
const BAR_GAP: f32 = 8.0;
const BAR_ICON: f32 = 14.0;
const PILL_PADDING_X: f32 = 8.0;
const PILL_PADDING_Y: f32 = 3.0;
const CONTENT_MARGIN: f32 = 8.0;
const CONTENT_PADDING: f32 = 10.0;
/// The feedback line every panel prints in: mono, four over the base size.
const FEEDBACK_SIZE_STEP: f32 = 4.0;
const FEEDBACK_LINE_HEIGHT: f32 = 1.5;
const PENDING_PANEL: &str = "arrives in a later PR";
/// The hover bubble a sidebar row carries, since the row truncates its
/// detail: the macOS `hoverBubble(width:trailing:)`, at a fixed spot right
/// of the row rather than under the pointer, and at once.
const BUBBLE_W: f32 = 240.0;
const BUBBLE_GAP: f32 = 8.0;
const BUBBLE_PADDING_X: f32 = 12.0;
const BUBBLE_PADDING_Y: f32 = 9.0;
const BUBBLE_LINE_HEIGHT: f32 = 1.35;

/// What each panel does on Enter, Tab and Escape, for the footer.
fn hint(id: &str) -> &'static str {
    match id {
        "pomo" => "Space: Start/pause \u{2022} R: Reset \u{2022} Esc: Back",
        "todo" => "Ctrl+Z/Shift+Z: Undo/Redo \u{2022} Ctrl+S: Save \u{2022} Esc: Back",
        "speed" => "R: Rerun \u{2022} E: Show IP \u{2022} Esc: Back",
        "kill" => "Y: Confirm \u{2022} N: Cancel \u{2022} Esc: Back",
        "sys" => "Esc: Back",
        "calc" => "Enter: Evaluate \u{2022} Tab: Select \u{2022} Esc: Back",
        _ => "Enter: Run \u{2022} Tab: Select \u{2022} Esc: Back",
    }
}

/// What the screen did with a key.
#[derive(PartialEq, Eq)]
pub enum KeyOutcome {
    Consumed,
    /// Not the screen's: the field or the launcher may want it.
    Pass,
    /// Escape with nothing to dismiss: close the screen.
    Exit,
}

pub struct Commands {
    active: usize,
    /// The sidebar row under the pointer, whose bubble shows.
    hovered: Option<usize>,
    input: Entity<SearchInput>,
    pub(super) shell: shell::Shell,
    pub(super) kill: kill::Kill,
    pub(super) calc: calc::Calc,
    pub(super) sys: sys::Sys,
    pub(super) pomo: pomo::Panel,
    pub(super) todo: todo::Panel,
}

impl Commands {
    pub fn new(input: Entity<SearchInput>) -> Self {
        Self {
            active: 0,
            hovered: None,
            input,
            shell: shell::Shell::default(),
            kill: kill::Kill::default(),
            calc: calc::Calc::default(),
            sys: sys::Sys::default(),
            pomo: pomo::Panel::default(),
            todo: todo::Panel::default(),
        }
    }

    pub fn entry(&self) -> &'static CommandEntry {
        &COMMAND_ENTRIES[self.active]
    }

    pub fn input(&self) -> &Entity<SearchInput> {
        &self.input
    }

    /// The command last shown, where Ctrl+/ reopens.
    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn hint(&self) -> &'static str {
        hint(self.entry().id)
    }

    /// Where the catalog puts `id`, if it names a command.
    pub fn index_of(id: &str) -> Option<usize> {
        COMMAND_ENTRIES.iter().position(|e| e.id == id)
    }

    /// Open on `index`, with `prefill` in the panel's box.
    pub fn enter(&mut self, index: usize, prefill: &str, cx: &mut Context<Launcher>) {
        self.active = index.min(COMMAND_ENTRIES.len() - 1);
        self.input
            .update(cx, |input, cx| input.set_text(prefill, cx));
        self.enter_panel(cx);
    }

    /// The screen closed, or the panel changed: what runs only while a
    /// panel is up stops.
    pub fn exit(&mut self) {
        self.pomo.leave();
        self.todo.leave();
    }

    /// A row's field open for typing, which the launcher's keys edit.
    pub fn editing_field(&self) -> Option<Entity<SearchInput>> {
        match self.entry().id {
            pomo::ID => self.pomo.editing_field(),
            todo::ID => self.todo.editing_field(),
            _ => None,
        }
    }

    /// Tab, Shift+Tab, Ctrl+digit, a sidebar click.
    fn switch_to(&mut self, index: usize, cx: &mut Context<Launcher>) {
        if index == self.active || index >= COMMAND_ENTRIES.len() {
            return;
        }
        self.exit();
        self.active = index;
        self.input.update(cx, |input, cx| input.clear(cx));
        self.enter_panel(cx);
    }

    fn enter_panel(&mut self, cx: &mut Context<Launcher>) {
        match self.entry().id {
            shell::ID => self.shell.enter(),
            kill::ID => self.kill.enter(cx),
            calc::ID => self.calc.enter(),
            sys::ID => self.sys.enter(cx),
            pomo::ID => self.pomo.enter(cx),
            todo::ID => self.todo.enter(cx),
            _ => {}
        }
        cx.notify();
    }

    /// The panel's box changed: the kill list filters and the calculator
    /// previews as it is typed.
    pub fn input_changed(&mut self, cx: &mut Context<Launcher>) {
        let typed = self.typed(cx);
        match self.entry().id {
            kill::ID => self.kill.filter(&typed, cx),
            calc::ID => {
                self.calc.preview(&typed);
                cx.notify();
            }
            // The list filters as the search box changes.
            todo::ID => cx.notify(),
            _ => {}
        }
    }

    /// What is typed in the panel's box.
    fn typed(&self, cx: &Context<Launcher>) -> String {
        self.input.read(cx).committed().trim().to_string()
    }

    /// The command screen's keys, before the launcher's own.
    pub fn handle_key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Launcher>) -> KeyOutcome {
        let ctrl = ks.modifiers.control;
        let shift = ks.modifiers.shift;
        let key = ks.key.as_str();
        if key == "escape" {
            let dismissed = match self.entry().id {
                kill::ID => self.kill.dismiss(cx),
                pomo::ID => self.pomo.dismiss(cx),
                todo::ID => self.todo.dismiss(cx),
                _ => false,
            };
            return if dismissed {
                KeyOutcome::Consumed
            } else {
                KeyOutcome::Exit
            };
        }
        // A row's field being typed into owns Tab.
        if self.editing_field().is_some() {
            return match self.entry().id {
                todo::ID => self.todo.key(ks, cx),
                _ => self.pomo.key(key, cx),
            };
        }
        if key == "tab" {
            let count = COMMAND_ENTRIES.len() as isize;
            let delta = if shift { -1 } else { 1 };
            let next = (self.active as isize + delta).rem_euclid(count) as usize;
            self.switch_to(next, cx);
            return KeyOutcome::Consumed;
        }
        if ctrl
            && !shift
            && let Some(digit) = key.chars().next().and_then(|c| c.to_digit(10))
            && (1..=COMMAND_ENTRIES.len() as u32).contains(&digit)
        {
            self.switch_to(digit as usize - 1, cx);
            return KeyOutcome::Consumed;
        }
        match self.entry().id {
            shell::ID if key == "enter" => {
                let command = self.typed(cx);
                self.shell.run(command, cx);
                KeyOutcome::Consumed
            }
            kill::ID => self.kill.key(key, cx),
            pomo::ID => self.pomo.key(key, cx),
            todo::ID => self.todo.key(ks, cx),
            calc::ID if key == "enter" => {
                let expr = self.typed(cx);
                self.calc.run(&expr, cx);
                KeyOutcome::Consumed
            }
            _ => KeyOutcome::Pass,
        }
    }

    /// The sidebar, the divider and the active panel, inside the card the
    /// launcher draws around them.
    pub fn render(&self, icons: &Entity<IconStore>, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let sidebar = div()
            .w(px(SIDEBAR_W))
            .flex_shrink_0()
            .p(px(SIDEBAR_PADDING))
            .flex()
            .flex_col()
            .gap(px(SIDEBAR_GAP))
            .children(COMMAND_ENTRIES.iter().enumerate().map(|(i, entry)| {
                let active = i == self.active;
                div()
                    .id(("cmd", i))
                    .px(px(SIDEBAR_ROW_PADDING_X))
                    .py(px(SIDEBAR_ROW_PADDING_Y))
                    .rounded(px(th.chip_radius()))
                    .when(active, |el| el.bg(th.selection_fill))
                    .hover(|s| s.bg(th.selection_fill))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.commands.switch_to(i, cx)))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        let commands = &mut this.commands;
                        if *hovered {
                            commands.hovered = Some(i);
                        } else if commands.hovered == Some(i) {
                            commands.hovered = None;
                        }
                        cx.notify();
                    }))
                    .relative()
                    .when(self.hovered == Some(i), |el| {
                        el.child(bubble(entry.detail, th))
                    })
                    .flex()
                    .items_center()
                    .gap(px(SIDEBAR_ROW_GAP))
                    .child(
                        div()
                            .size(px(SIDEBAR_ICON_BOX))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                svg()
                                    .path(entry.glyph)
                                    .size(px(SIDEBAR_ICON))
                                    .text_color(th.accent),
                            ),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(th.font_size - 1.0))
                                    .line_height(px((th.font_size - 1.0) * SIDEBAR_LINE_HEIGHT))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("/{}", entry.id)),
                            )
                            .child(
                                div()
                                    .text_size(px(th.font_size - 2.0))
                                    .line_height(px((th.font_size - 2.0) * SIDEBAR_LINE_HEIGHT))
                                    .text_color(th.text_secondary)
                                    .truncate()
                                    .child(entry.detail),
                            ),
                    )
            }));
        let divider = div()
            .w(px(1.0))
            .flex_shrink_0()
            .my(px(DIVIDER_MARGIN))
            .bg(th.border);
        let panel: AnyElement = match self.entry().id {
            shell::ID => shell::panel(self, th).into_any_element(),
            kill::ID => kill::panel(self, icons, th, cx).into_any_element(),
            calc::ID => calc::panel(self, th).into_any_element(),
            sys::ID => sys::panel(self, th).into_any_element(),
            pomo::ID => pomo::panel(self, th, cx).into_any_element(),
            todo::ID => todo::panel(self, th, cx).into_any_element(),
            _ => self.pending_panel(th).into_any_element(),
        };
        // The pomo at rest keeps only its ring.
        let faded = self.entry().id == pomo::ID && self.pomo.is_idle();
        div()
            .size_full()
            .flex()
            .when(th.split() && !faded, |el| el.child(sidebar).child(divider))
            .child(div().flex_1().min_w_0().flex().flex_col().child(panel))
    }

    /// The box every panel with input has: the command's glyph, the field,
    /// the `/name` pill.
    fn input_bar(&self, placeholder: &'static str, th: &Theme) -> Div {
        self.bar(th)
            .child(search_field(self.input.clone(), placeholder))
            .child(self.pill(th))
    }

    /// The bar's frame: glyph first, the rest is the panel's.
    fn bar(&self, th: &Theme) -> Div {
        div()
            .mt(px(BAR_MARGIN_TOP))
            .mx(px(BAR_MARGIN_X))
            .px(px(BAR_PADDING_X))
            .py(px(BAR_PADDING_Y))
            .rounded(px(th.bar_radius()))
            .bg(th.control_fill)
            .flex()
            .items_center()
            .gap(px(BAR_GAP))
            .child(
                svg()
                    .path(self.entry().glyph)
                    .size(px(BAR_ICON))
                    .text_color(th.accent),
            )
    }

    fn pill(&self, th: &Theme) -> Div {
        div()
            .flex_shrink_0()
            .px(px(PILL_PADDING_X))
            .py(px(PILL_PADDING_Y))
            .rounded_full()
            .bg(th.selection_fill)
            .font_family(th.mono_family.clone())
            .text_size(px(th.font_size - 1.0))
            .child(format!("/{}", self.entry().id))
    }

    /// Where a panel's output goes, under its bar.
    fn content(&self) -> Div {
        div()
            .flex_1()
            .min_h_0()
            .mx(px(CONTENT_MARGIN))
            .mb(px(CONTENT_MARGIN))
            .p(px(CONTENT_PADDING))
            .flex()
            .flex_col()
    }

    /// The feedback line's type.
    fn feedback(&self, th: &Theme) -> Div {
        let size = th.font_size + FEEDBACK_SIZE_STEP;
        div()
            .font_family(th.mono_family.clone())
            .text_size(px(size))
            .line_height(px(size * FEEDBACK_LINE_HEIGHT))
            .font_weight(FontWeight::SEMIBOLD)
    }

    /// A command whose panel is still to come, in its own PR.
    fn pending_panel(&self, th: &Theme) -> Div {
        let entry = self.entry();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                self.bar(th)
                    .child(
                        div()
                            .flex_1()
                            .text_color(th.text_secondary)
                            .child(entry.detail),
                    )
                    .child(self.pill(th)),
            )
            .child(
                self.content().child(
                    div()
                        .text_color(th.text_muted)
                        .child(format!("/{} {PENDING_PANEL}", entry.id)),
                ),
            )
    }
}

/// A row's full detail, trailing the row and centred on it. Deferred, so it
/// paints over the panel beside the sidebar.
fn bubble(text: &'static str, th: &Theme) -> impl IntoElement {
    let row_w = SIDEBAR_W - 2.0 * SIDEBAR_PADDING;
    deferred(
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(row_w + BUBBLE_GAP))
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(BUBBLE_W))
                    .px(px(BUBBLE_PADDING_X))
                    .py(px(BUBBLE_PADDING_Y))
                    .rounded(px(th.chip_radius()))
                    .bg(th.card_face())
                    .border(px(1.0))
                    .border_color(th.border)
                    .shadow(th.card_shadow())
                    .text_size(px(th.font_size - 2.0))
                    .line_height(px((th.font_size - 2.0) * BUBBLE_LINE_HEIGHT))
                    .text_color(th.text)
                    .child(text),
            ),
    )
}

/// `:cmd <args>`: the inline trigger. Bare `:calc` keeps the menu open; only
/// whitespace after a known id flips into its panel.
pub fn inline_command(query: &str) -> Option<(usize, &str)> {
    let rest = query.strip_prefix(':')?;
    let space = rest.find(char::is_whitespace)?;
    let index = Commands::index_of(&rest[..space].to_lowercase())?;
    Some((index, rest[space + 1..].trim_start()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_commands_need_a_space() {
        assert_eq!(inline_command(":calc"), None);
        assert_eq!(inline_command(":calc 2+2"), Some((0, "2+2")));
        assert_eq!(inline_command(":shell  ls -la"), Some((5, "ls -la")));
        assert_eq!(inline_command(":nope x"), None);
        assert_eq!(inline_command("calc 2"), None);
    }

    #[test]
    fn every_command_has_a_hint() {
        for entry in COMMAND_ENTRIES {
            assert!(hint(entry.id).contains("Esc"));
        }
    }

    #[test]
    fn the_panel_ids_are_in_the_catalog() {
        for id in [shell::ID, kill::ID, calc::ID, sys::ID, pomo::ID] {
            assert!(Commands::index_of(id).is_some(), "{id}");
        }
    }
}
