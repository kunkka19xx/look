//! The Extend tab: what Look has been extended with, the blocks you wrote
//! and the ones an app installed, and where one goes off or away. The
//! macOS `ExtendSettingsTab`.

use std::collections::HashSet;
use std::path::Path;

use gpui::prelude::*;
use gpui::{Context, Div, ElementId, FontWeight, Rgba, SharedString, Stateful, div, px, svg};
use linows_backend::look_engine::config_path;
use linows_backend::look_engine::sources::{InstalledBlock, Inventory, Reported};
use linows_backend::look_sources;
use linows_backend::sources::RowArgs;
use linows_backend::{sources, tools, trash};

use super::Settings;
use crate::Shell;
use crate::actions;
use crate::bg;
use crate::glyphs;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::theme::Theme;

const UNREADABLE: &str = "Could not read what is installed.";
const APPS: Section = Section {
    title: "Installed by apps",
    at: "~/.look/connectors",
    empty: "No app has installed anything yet.",
    from_app: true,
};
const YOURS: Section = Section {
    title: "Yours",
    at: "~/.look/sources",
    empty: "Nothing declared yet.",
    from_app: false,
};
const NO_PROBLEMS: &str = "No problems.";
const TURN_ON: &str = "Turn on";
const TURN_OFF: &str = "Turn off";
const EDIT: &str = "Edit";
const REVEAL: &str = "Reveal";
const REMOVE: &str = "Remove";
const KIND_CONNECT: &str = "connect";
const STATE_OFF: &str = "off";
const STATE_RUNNING: &str = "running";
const STATE_STOPPED: &str = "not running";
/// Complaints shown under a row before the list stops being a list.
const LOG_LINES: usize = 3;
const SECTION_GAP: f32 = 14.0;
const SECTION_PADDING_TOP: f32 = 8.0;
const ROW_GAP: f32 = 8.0;
const HEADER_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 10.0;
const CARD_GAP: f32 = 4.0;
const BLOCK_GAP: f32 = 8.0;
const NEED_GAP: f32 = 2.0;
const NEED_INDENT: f32 = 12.0;
const PILL_PADDING_X: f32 = 9.0;
const PILL_PADDING_Y: f32 = 3.0;
const WARNING_GLYPH: f32 = 12.0;
/// Long enough to read a config key out of.
const FAILURE_SECONDS: f32 = 4.0;

/// One of the two lists: its heading, where its files live, and what it
/// says when there are none.
struct Section {
    title: &'static str,
    at: &'static str,
    empty: &'static str,
    from_app: bool,
}

/// The three states this screen has. An unread inventory must not render
/// as an empty one: every line below is a claim about what is installed.
#[derive(Default)]
enum Reading {
    #[default]
    Pending,
    Unreadable,
    Ready(Inventory),
}

#[derive(Default)]
pub(super) struct Extend {
    reading: Reading,
    /// Bumped by every read, so an older answer cannot land last.
    reads: u64,
}

/// Why a pill did nothing, in the banner: this screen's whole job is saying
/// what is installed and what is not, so a failure is not swallowed.
fn failed(launcher: &mut Launcher, text: String, cx: &mut Context<Launcher>) {
    launcher.banner.show(text, Tone::Error, FAILURE_SECONDS, cx);
}

impl Extend {
    /// A fresh settings session: nothing read yet, and a read still in
    /// flight from the last session is disowned.
    pub(super) fn forget(&mut self) {
        self.reading = Reading::Pending;
        self.reads += 1;
    }

    pub(super) fn read(&mut self, cx: &mut Context<Launcher>) {
        self.reads += 1;
        let read = self.reads;
        bg::fetch(cx, sources::inventory, move |this, found, _| {
            let extend = &mut this.settings.extend;
            if extend.reads == read {
                extend.reading = found.map_or(Reading::Unreadable, Reading::Ready);
            }
        });
    }

    /// Writes `on` into every block of the file, then reloads the sources:
    /// the bytes are on disk and nothing has read them, and the reload is
    /// what stops a connector just turned off.
    fn set_enabled(&mut self, file: String, on: bool, cx: &mut Context<Launcher>) {
        bg::fetch(
            cx,
            move || (sources::set_enabled(Path::new(&file), on), file),
            |this, (switched, file), cx| match switched {
                Some(switched) if switched.problems.is_empty() => {
                    if switched.changed {
                        this.reload_sources(cx);
                    }
                }
                Some(switched) => failed(this, switched.problems.join("\n"), cx),
                None => failed(this, format!("Could not change {}", file_name(&file)), cx),
            },
        );
    }

    /// To the trash, unconfirmed: the trash is the undo.
    fn remove(&mut self, file: String, cx: &mut Context<Launcher>) {
        bg::fetch(
            cx,
            move || trash::trash_paths(vec![file]),
            |this, outcome, cx| match outcome.failed.first() {
                Some(failure) => failed(
                    this,
                    format!(
                        "Could not remove {}: {}",
                        file_name(&failure.path),
                        failure.reason
                    ),
                    cx,
                ),
                None => this.reload_sources(cx),
            },
        );
    }

    /// Edit or Reveal through the declared tools, as Ctrl+E and Ctrl+F on a
    /// row are. The action hides the launcher itself.
    fn tool(
        &mut self,
        action: &'static str,
        file: String,
        shell: Shell,
        cx: &mut Context<Launcher>,
    ) {
        let args = RowArgs {
            candidate_id: String::new(),
            row_title: file_name(&file).to_string(),
            row_path: file,
            query: String::new(),
            ancestors: String::new(),
        };
        bg::fetch(
            cx,
            move || tools::perform_tool_action(&shell, action, args, Some(false)),
            |this, outcome, cx| {
                if let Some(reason) = outcome.and_then(|r| r.reason) {
                    failed(this, reason, cx);
                }
            },
        );
    }
}

/// One file with the blocks it declared: the unit a user installs and
/// deletes, so the unit this lists.
struct Declaration<'a> {
    file: &'a str,
    blocks: Vec<&'a InstalledBlock>,
    problems: Vec<&'a Reported>,
}

impl Declaration<'_> {
    /// A file that parsed to nothing has no block to say who wrote it, so
    /// its directory answers, by the rule core applies.
    fn installed_by_app(&self, connectors: Option<&Path>) -> bool {
        self.blocks.first().map_or_else(
            || connectors.is_some_and(|dir| Path::new(self.file).starts_with(dir)),
            |b| b.from_app,
        )
    }

    /// Blocks that disagree read as on, so one press turns the rest off.
    fn off(&self) -> bool {
        !self.blocks.is_empty() && self.blocks.iter().all(|b| !b.enabled)
    }
}

/// By file, in first-seen order. A file that failed to parse has no blocks
/// and is listed from its problems alone: it is the one worth showing.
fn declarations(inventory: &Inventory) -> Vec<Declaration<'_>> {
    let mut out: Vec<Declaration> = Vec::new();
    for block in &inventory.blocks {
        slot(&mut out, &block.file).blocks.push(block);
    }
    for problem in &inventory.problems {
        slot(&mut out, &problem.file).problems.push(problem);
    }
    out
}

fn slot<'a, 'd>(out: &'d mut Vec<Declaration<'a>>, file: &'a str) -> &'d mut Declaration<'a> {
    let at = out.iter().position(|d| d.file == file).unwrap_or_else(|| {
        out.push(Declaration {
            file,
            blocks: Vec::new(),
            problems: Vec::new(),
        });
        out.len() - 1
    });
    &mut out[at]
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Only a `connect` block has a process to be up or down.
///
/// A connector that is down says why: "not running" alone sends somebody to a
/// log they cannot reach.
fn state_of(block: &InstalledBlock) -> String {
    if !block.enabled {
        STATE_OFF.to_string()
    } else if block.kind != KIND_CONNECT {
        String::new()
    } else if block.running {
        STATE_RUNNING.to_string()
    } else {
        match block.log.last() {
            Some(last) => format!("{STATE_STOPPED}: {last}"),
            None => STATE_STOPPED.to_string(),
        }
    }
}

impl Settings {
    /// The two installed lists with the lookbook under them, or the sheet
    /// reading one entry in their place.
    pub(super) fn extend(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let body = div()
            .pt(px(SECTION_PADDING_TOP))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP));
        if self.sheet_open() {
            return body.child(self.sheet(th, cx));
        }
        let mut present = HashSet::new();
        let body = match &self.extend.reading {
            Reading::Pending => body,
            Reading::Unreadable => body.child(warning(UNREADABLE, th)),
            Reading::Ready(inventory) => {
                body.child(self.extend_sections(inventory, &mut present, th, cx))
            }
        };
        body.child(self.browse_section(&present, th, cx))
    }

    /// `present` collects the file names in the sources directory, which
    /// is what tells the lookbook an entry is already here.
    fn extend_sections<'a>(
        &self,
        inventory: &'a Inventory,
        present: &mut HashSet<&'a str>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let declarations = declarations(inventory);
        let connectors =
            config_path::home().map(|home| look_sources::connectors_dir(Path::new(&home)));
        let (apps, yours): (Vec<_>, Vec<_>) = declarations
            .iter()
            .enumerate()
            .partition(|(_, d)| d.installed_by_app(connectors.as_deref()));
        present.extend(yours.iter().map(|(_, d)| file_name(d.file)));
        let clean = inventory.problems.is_empty()
            && inventory.blocks.iter().all(|block| block.needs.is_empty());
        div()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(self.extend_section(&APPS, &apps, th, cx))
            .child(self.extend_section(&YOURS, &yours, th, cx))
            .when(clean, |el| {
                el.child(muted(NO_PROBLEMS, th.font_size - 3.0, th))
            })
    }

    fn extend_section(
        &self,
        section: &Section,
        items: &[(usize, &Declaration)],
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let Section {
            title,
            at,
            empty,
            from_app,
        } = *section;
        let header = div()
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(th.text)
                    .child(title),
            )
            .child(
                div()
                    .font_family(th.mono_family.clone())
                    .text_size(px(th.font_size - 3.0))
                    .text_color(th.text_muted)
                    .child(at),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .child(header)
            .when(items.is_empty(), |el| {
                el.child(muted(empty, th.font_size - 2.0, th))
            })
            .children(items.iter().map(|(index, declaration)| {
                self.declaration_card(*index, declaration, from_app, th, cx)
            }))
    }

    fn declaration_card(
        &self,
        index: usize,
        declaration: &Declaration,
        from_app: bool,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let file = declaration.file.to_string();
        let header = div()
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(th.mono_family.clone())
                    .text_size(px(th.font_size - 2.0))
                    .text_color(th.text_secondary)
                    .child(SharedString::from(file_name(&file).to_string())),
            )
            // Nothing parsed means no block to carry the key, so there is
            // nothing to turn off. Removing it is still the way out.
            .when(!declaration.blocks.is_empty(), |el| {
                let on = declaration.off();
                let path = file.clone();
                el.child(
                    pill(
                        pill_id("extend-switch", index),
                        if on { TURN_ON } else { TURN_OFF },
                        None,
                        th,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.extend.set_enabled(path.clone(), on, cx);
                    })),
                )
            })
            .child({
                let path = file.clone();
                pill(pill_id("extend-edit", index), EDIT, None, th).on_click(cx.listener(
                    move |this, _, _, cx| {
                        let shell = this.shell.clone();
                        this.settings
                            .extend
                            .tool(actions::EDIT, path.clone(), shell, cx);
                    },
                ))
            })
            .child({
                let path = file.clone();
                pill(pill_id("extend-reveal", index), REVEAL, None, th).on_click(cx.listener(
                    move |this, _, _, cx| {
                        let shell = this.shell.clone();
                        this.settings
                            .extend
                            .tool(actions::REVEAL, path.clone(), shell, cx);
                    },
                ))
            })
            // Only what the user wrote: an app would write its manifest back.
            .when(!from_app, |el| {
                let path = file.clone();
                el.child(
                    pill(
                        pill_id("extend-remove", index),
                        REMOVE,
                        Some(th.warning),
                        th,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.extend.remove(path.clone(), cx);
                    })),
                )
            });
        div()
            .p(px(CARD_PADDING))
            .rounded(px(th.control_radius()))
            .bg(th.control_fill)
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .child(header)
            .children(
                declaration
                    .problems
                    .iter()
                    .map(|problem| problem_line(&problem.message, th)),
            )
            .children(
                declaration
                    .blocks
                    .iter()
                    .map(|block| block_lines(block, th)),
            )
    }
}

pub(super) fn pill_id(verb: &'static str, index: usize) -> ElementId {
    ElementId::NamedInteger(verb.into(), index as u64)
}

/// One verb in a pill of its own. Bordered, so four in a row read apart,
/// and `tint` says which one is the odd verb.
pub(super) fn pill(
    id: ElementId,
    label: impl Into<SharedString>,
    tint: Option<Rgba>,
    th: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_shrink_0()
        .px(px(PILL_PADDING_X))
        .py(px(PILL_PADDING_Y))
        .rounded_full()
        .border(px(1.0))
        .border_color(th.border)
        .bg(th.selection_fill)
        // The settings buttons' size and weight: a step smaller loses the
        // stem of a narrow glyph between pixels.
        .text_size(px(th.font_size - 2.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tint.unwrap_or(th.accent))
        .cursor_pointer()
        .hover(|s| s.bg(th.panel_fill))
        .child(label.into())
}

fn block_lines(block: &InstalledBlock, th: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(NEED_GAP))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(BLOCK_GAP))
                .text_size(px(th.font_size - 2.0))
                .child(
                    div()
                        .text_color(if block.enabled {
                            th.text
                        } else {
                            th.text_muted
                        })
                        .child(SharedString::from(block.name.clone())),
                )
                .child(
                    div()
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size - 3.0))
                        .text_color(th.text_muted)
                        .child(block.kind),
                )
                .child(div().flex_1())
                .child(div().text_color(th.text_muted).child(state_of(block))),
        )
        // Indented: a line to change in that block, not the file as a whole.
        .children(block.needs.iter().map(|need| {
            div()
                .pl(px(NEED_INDENT))
                .text_size(px(th.font_size - 3.0))
                .text_color(th.warning)
                .child(SharedString::from(need.message.clone()))
        }))
        // Newest first, unlike the engine's order: the last complaint is the
        // one worth reading.
        .children(
            block
                .log
                .iter()
                .rev()
                .take(LOG_LINES)
                .map(|line| {
                    div()
                        .pl(px(NEED_INDENT))
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size - 3.0))
                        .text_color(th.text_muted)
                        .child(SharedString::from(line.clone()))
                })
                .collect::<Vec<_>>(),
        )
}

fn problem_line(message: &str, th: &Theme) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(HEADER_GAP))
        .text_size(px(th.font_size - 2.0))
        .text_color(th.warning)
        .child(
            svg()
                .path(glyphs::TRIANGLE_ALERT)
                .size(px(WARNING_GLYPH))
                .flex_shrink_0()
                .mt(px(NEED_GAP))
                .text_color(th.warning),
        )
        .child(SharedString::from(message.to_string()))
}

fn warning(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .text_size(px(th.font_size - 2.0))
        .text_color(th.warning)
        .child(text.into())
}

fn muted(text: &'static str, size: f32, th: &Theme) -> Div {
    div()
        .text_size(px(size))
        .text_color(th.text_muted)
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(file: &str, enabled: bool, from_app: bool) -> InstalledBlock {
        InstalledBlock {
            id: String::new(),
            name: "b".into(),
            icon: None,
            kind: KIND_CONNECT,
            enabled,
            file: file.into(),
            from_app,
            running: false,
            needs: Vec::new(),
            log: Vec::new(),
        }
    }

    #[test]
    fn a_file_is_listed_once_and_an_unparsed_one_still_listed() {
        let inventory = Inventory {
            blocks: vec![
                block("/h/.look/sources/a.toml", true, false),
                block("/h/.look/sources/a.toml", false, false),
            ],
            problems: vec![Reported {
                file: "/h/.look/connectors/broken.toml".into(),
                message: "bad".into(),
            }],
        };
        let listed = declarations(&inventory);
        let connectors = Some(Path::new("/h/.look/connectors"));
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].blocks.len(), 2);
        assert!(!listed[0].off(), "blocks that disagree read as on");
        assert!(!listed[0].installed_by_app(connectors));
        assert!(listed[1].blocks.is_empty());
        assert_eq!(listed[1].problems.len(), 1);
        assert_eq!(listed[1].problems[0].message, "bad");
        assert!(listed[1].installed_by_app(connectors));
        assert!(!listed[1].installed_by_app(None), "no home, no claim");
    }

    #[test]
    fn only_a_connect_block_has_a_running_state() {
        let mut b = block("/f", true, false);
        assert_eq!(state_of(&b), STATE_STOPPED);
        b.running = true;
        assert_eq!(state_of(&b), STATE_RUNNING);
        b.kind = "dir";
        assert_eq!(state_of(&b), "");
        b.enabled = false;
        assert_eq!(state_of(&b), STATE_OFF);
    }

    #[test]
    fn a_connector_that_is_down_says_what_it_said() {
        // The state this screen exists to explain.
        let mut b = block("/f", true, false);
        b.log = vec!["first".into(), "spawn failed: no such file".into()];
        assert_eq!(
            state_of(&b),
            format!("{STATE_STOPPED}: spawn failed: no such file")
        );
        // Up again, and the old complaint is not news.
        b.running = true;
        assert_eq!(state_of(&b), STATE_RUNNING);
    }
}
