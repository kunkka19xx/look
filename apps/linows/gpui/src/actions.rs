//! The verbs a row offers, the chords that carry them, and the Ctrl+K menu
//! that lists them. One table and one eligibility rule, read by both, so the
//! two cannot drift. Composition lives in core (`look-tools`), and the
//! wording of an action that cannot run comes back with it.
//!
//! A row a block produced also carries what the block declared: its own
//! `edit` / `terminal` / `reveal` beat the user's global tool, and its `then`
//! targets join this list. Any row at all can carry an action a `do` block
//! declared with `applies`, whatever produced the row.

use gpui::{Context, Div, FontWeight, div, prelude::*, px};
use linows_backend::sources::{self, RowArgs};
use linows_backend::tools;

use crate::launcher::Launcher;
use crate::rows::Row;
use crate::theme::{self, Theme};

/// Core's action ids (`look_tools::Action::id`).
pub const EDIT: &str = "edit";
pub const TERMINAL: &str = "terminal";
pub const REVEAL: &str = "reveal";
/// `look_indexing::CandidateIdKind::PREFIX_SOURCE`: a row a block produced.
pub const SOURCE_PREFIX: &str = "src:";
/// How long an action's explanation stays up.
pub const BANNER_SECONDS: f32 = 2.4;
/// The menu's own, short ones.
pub const MENU_BANNER_SECONDS: f32 = 1.2;
pub const EMPTY: &str = "Nothing to do here";
const CANCEL: &str = "Cancel";
const CONFIRM_FALLBACK: &str = "Confirm?";
/// The platform's own file manager, for "Reveal in Files".
#[cfg(target_os = "linux")]
const SYSTEM_FILE_MANAGER: &str = "Files";
#[cfg(not(target_os = "linux"))]
const SYSTEM_FILE_MANAGER: &str = "Explorer";

const MENU_TOP: f32 = 84.0;
const MENU_PADDING: f32 = 6.0;
/// macOS `ActionMenu.compactWidth` and `compactInset`.
const COMPACT_W: f32 = 320.0;
const COMPACT_INSET: f32 = 10.0;
const ROW_PADDING_X: f32 = 10.0;
const ROW_PADDING_Y: f32 = 7.0;
const ROW_GAP: f32 = 16.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionId {
    Open,
    Tool(&'static str),
    CopyPath,
    OpenAllPicked,
    ClearPicked,
    DeleteClipboard,
    /// A block's `then` target, by block id.
    Target(String),
    Yes,
    No,
}

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub id: ActionId,
    pub title: String,
    pub chord: &'static str,
    /// Asked in the menu before the action runs.
    pub confirm: Option<String>,
}

/// One catalog entry: the id, the plain wording, the wording once a tool
/// resolves ("Edit" becomes "Edit in Zed"), the chord, and the core action.
struct Entry {
    id: ActionId,
    plain: &'static str,
    named: Option<&'static str>,
    chord: &'static str,
    tool: Option<&'static str>,
    /// Stands in for a declared tool where the platform always has an answer.
    fallback_tool: Option<&'static str>,
}

const CATALOG: &[Entry] = &[
    Entry {
        id: ActionId::Open,
        plain: "Open",
        named: None,
        chord: "\u{23ce}",
        tool: None,
        fallback_tool: None,
    },
    Entry {
        id: ActionId::Tool(EDIT),
        plain: "Edit",
        named: Some("Edit in"),
        chord: "Ctrl+E",
        tool: Some(EDIT),
        fallback_tool: None,
    },
    Entry {
        id: ActionId::Tool(TERMINAL),
        plain: "Open terminal here",
        named: Some("Open in"),
        chord: "Ctrl+T",
        tool: Some(TERMINAL),
        fallback_tool: None,
    },
    Entry {
        id: ActionId::Tool(REVEAL),
        plain: "Reveal",
        named: Some("Reveal in"),
        chord: "Ctrl+F",
        tool: Some(REVEAL),
        fallback_tool: Some(SYSTEM_FILE_MANAGER),
    },
    Entry {
        id: ActionId::CopyPath,
        plain: "Copy path",
        named: None,
        chord: "Ctrl+C",
        tool: None,
        fallback_tool: None,
    },
];

pub fn is_source_row(id: &str) -> bool {
    id.starts_with(SOURCE_PREFIX)
}

/// `src:<block>:<row>` and the drilled `src:<block>:|…|<row>` alike to
/// `<block>`. The one place the shell reads a candidate id, and only its
/// namespace.
pub fn block_id_of(id: &str) -> Option<&str> {
    let rest = id.strip_prefix(SOURCE_PREFIX)?;
    let separator = rest.find(':')?;
    (separator > 0).then(|| &rest[..separator])
}

/// Whether `action` means anything for a row of this kind. Editing and a
/// terminal are about a place you work in; an app is a thing you launch,
/// so only revealing applies to it.
pub fn applies(action: &str, kind: &str) -> bool {
    let place = kind == "file" || kind == "folder" || kind == "action";
    if action == REVEAL {
        place || kind == "app"
    } else {
        place
    }
}

/// Whether the row is something actions apply to at all.
pub fn actionable(row: &Row) -> bool {
    if row.is_hint()
        || row.kind == crate::rows::KIND_CLIPBOARD
        || row.kind == crate::rows::KIND_PROCESS
    {
        return false;
    }
    !row.path.is_empty() || is_source_row(&row.id)
}

/// The row as core needs it: its id so a block can override the chord, its
/// title and ancestors so that override expands like any other command.
pub fn row_args(row: &Row, ancestors: &str) -> RowArgs {
    RowArgs {
        candidate_id: row.id.clone(),
        row_title: row.title.clone(),
        row_path: row.path.clone(),
        query: String::new(),
        ancestors: ancestors.to_string(),
    }
}

/// A file row and a folder row say which they are; a block's row does not,
/// so only the filesystem knows and the backend is what asks it.
pub fn is_dir(row: &Row) -> Option<bool> {
    if row.kind == "action" {
        None
    } else {
        Some(row.kind == "folder")
    }
}

/// What the Ctrl+K menu lists for a row. Blocks first: a block that
/// declared `then` targets shows those instead of the verb list; an action
/// declared for rows like this one joins at the end. Resolves the tool names
/// in one call. Blocking, so it runs on the background executor.
pub fn descriptors(row: &Row, ancestors: String, panel: Vec<MenuItem>) -> Vec<MenuItem> {
    let block = sources::source_block(row_args(row, &ancestors));
    let describe = |targets: &[linows_backend::look_engine::sources::ThenTarget]| {
        targets
            .iter()
            .map(|target| MenuItem {
                id: ActionId::Target(target.id.clone()),
                // The ellipsis says this one lists rather than runs.
                title: if target.performs {
                    target.name.clone()
                } else {
                    format!("{}\u{2026}", target.name)
                },
                chord: "",
                confirm: target.confirm.clone(),
            })
            .collect::<Vec<_>>()
    };
    let targets = block
        .as_ref()
        .map(|b| describe(&b.then))
        .unwrap_or_default();
    let declared = block
        .as_ref()
        .map(|b| describe(&b.globals))
        .unwrap_or_default();
    let mut items = panel;
    if !targets.is_empty() {
        items.extend(targets);
        items.extend(declared);
        return items;
    }
    if row.path.is_empty() {
        items.extend(declared);
        return items;
    }

    let offered: Vec<&Entry> = CATALOG
        .iter()
        .filter(|entry| entry.tool.is_none_or(|tool| applies(tool, &row.kind)))
        .collect();
    let asked: Vec<String> = offered
        .iter()
        .filter_map(|entry| entry.tool.map(str::to_string))
        .collect();
    let resolved = if asked.is_empty() {
        Vec::new()
    } else {
        tools::tool_actions(&asked, row_args(row, &ancestors), is_dir(row))
    };
    let tool_name = |action: &str| {
        asked
            .iter()
            .position(|a| a == action)
            .and_then(|at| resolved.get(at))
            .and_then(|r| r.as_ref())
            .and_then(|r| r.tool.clone())
    };
    items.extend(offered.iter().map(|entry| {
        let named = entry
            .tool
            .and_then(tool_name)
            .or_else(|| entry.fallback_tool.map(str::to_string));
        let title = match (entry.named, named) {
            (Some(prefix), Some(tool)) => format!("{prefix} {tool}"),
            _ => entry.plain.to_string(),
        };
        MenuItem {
            id: entry.id.clone(),
            title,
            chord: entry.chord,
            confirm: None,
        }
    }));
    items.extend(declared);
    items
}

/// The split layout's side panels, offered in the menu where the compact
/// layout has no room for them.
pub fn panel_items(has_picked: bool, on_clip: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if has_picked {
        items.push(MenuItem {
            id: ActionId::OpenAllPicked,
            title: "Open picked".into(),
            chord: "Shift+Enter",
            confirm: None,
        });
        items.push(MenuItem {
            id: ActionId::ClearPicked,
            title: "Clear picked".into(),
            chord: "Ctrl+Shift+P",
            confirm: None,
        });
    }
    if on_clip {
        items.push(MenuItem {
            id: ActionId::DeleteClipboard,
            title: "Delete from history".into(),
            chord: "Ctrl+D",
            confirm: None,
        });
    }
    items
}

/// The menu while it is up: its rows, the one the keys are on, and the
/// action waiting on a yes while it asks.
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub focused: usize,
    pub pending: Option<ActionId>,
}

impl Menu {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            focused: 0,
            pending: None,
        }
    }

    /// Swap to the question: picking it is the only yes, and focus starts on
    /// Cancel so a destructive action costs one more press than a double
    /// Enter.
    pub fn ask(&mut self, question: Option<String>, action: ActionId) {
        self.items = vec![
            MenuItem {
                id: ActionId::Yes,
                title: question.unwrap_or_else(|| CONFIRM_FALLBACK.to_string()),
                chord: "",
                confirm: None,
            },
            MenuItem {
                id: ActionId::No,
                title: CANCEL.into(),
                chord: "",
                confirm: None,
            },
        ];
        self.focused = 1;
        self.pending = Some(action);
    }

    /// Wraps at both ends, so holding one direction cycles.
    pub fn move_by(&mut self, delta: isize) {
        let count = self.items.len() as isize;
        if count > 0 {
            self.focused = (self.focused as isize + delta).rem_euclid(count) as usize;
        }
    }

    /// The popover: over the preview column in the split layout; in the
    /// compact one, over the list, centred on its right edge and opaque so
    /// the rows under it do not read through.
    pub fn render(&self, split: bool, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let rows = self.items.iter().enumerate().map(|(i, item)| {
            let focused = i == self.focused;
            div()
                .id(("action", i))
                .px(px(ROW_PADDING_X))
                .py(px(ROW_PADDING_Y))
                .rounded(px(th.chip_radius()))
                .when(focused, |el| el.bg(th.selection_fill))
                .hover(|s| s.bg(th.selection_fill))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.activate_menu_item(i, cx)))
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(th.font_size - 1.0))
                        .when(focused, |el| el.font_weight(FontWeight::SEMIBOLD))
                        .truncate()
                        .child(item.title.clone()),
                )
                .when(!item.chord.is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(px(th.font_size - 3.0))
                            .text_color(th.text_muted)
                            .child(item.chord),
                    )
                })
        });
        let menu = div()
            .p(px(MENU_PADDING))
            .rounded(px(th.control_radius()))
            .border(px(th.border_thickness))
            .border_color(th.border)
            .shadow(th.card_shadow())
            .flex()
            .flex_col()
            .children(rows);
        if !split {
            return div()
                .absolute()
                .inset_0()
                .pr(px(COMPACT_INSET))
                .flex()
                .items_center()
                .justify_end()
                .child(menu.w(px(COMPACT_W)).bg(theme::opaque(th.card_face())));
        }
        let column_w = (theme::WINDOW_W - 2.0 * theme::CONTENT_PADDING - th.inner_gap) / 2.0;
        menu.absolute()
            .top(px(MENU_TOP))
            .right(px(theme::CONTENT_PADDING))
            .w(px(column_w - 2.0 * theme::CONTENT_PADDING))
            .bg(th.card_face())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_ids_come_out_of_source_rows() {
        assert_eq!(block_id_of("src:branches:main"), Some("branches"));
        assert_eq!(block_id_of("src:branches:|a|main"), Some("branches"));
        assert_eq!(block_id_of("app:/x.desktop"), None);
        assert_eq!(block_id_of("src::x"), None);
    }

    #[test]
    fn apps_only_reveal() {
        assert!(applies(REVEAL, "app"));
        assert!(!applies(EDIT, "app"));
        assert!(applies(EDIT, "folder"));
        assert!(applies(TERMINAL, "action"));
    }

    #[test]
    fn the_question_starts_on_cancel() {
        let mut menu = Menu::new(Vec::new());
        menu.ask(Some("Delete main?".into()), ActionId::Target("b".into()));
        assert_eq!(menu.focused, 1);
        assert_eq!(menu.items[0].id, ActionId::Yes);
        menu.move_by(1);
        assert_eq!(menu.focused, 0);
    }
}
