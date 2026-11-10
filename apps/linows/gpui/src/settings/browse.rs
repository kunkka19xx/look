//! Browse lookbook: ready-made sources listed under what is installed, and
//! the sheet that reads one before writing it. The macOS
//! `LookbookBrowseSection` and `LookbookInstallSheet`.
//!
//! Nothing is fetched until the Extend tab shows: the catalog is a
//! deliberate network call, not something Look does on launch.

use std::collections::HashSet;

use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, HighlightStyle, SharedString, StyledText,
    UnderlineStyle, div, px,
};
use linows_backend::look_engine::lookbook::{
    Field as Setup, InstallRequest, PresentRequest, Preview,
};
use linows_backend::lookbook::{self, Catalog, Entry};

use super::extend::{pill, pill_id};
use super::{Field, FieldKind, Settings};
use crate::bg;
use crate::controls;
use crate::launcher::Launcher;
use crate::pick;
use crate::search::{Changed, input_for, search_field};
use crate::theme::Theme;

const TITLE: &str = "Browse lookbook";
const SUBTITLE: &str = "ready-made sources to copy in";
const FILTER_PLACEHOLDER: &str = "Filter";
const READING_CATALOG: &str = "Reading the catalog\u{2026}";
const READING_ENTRY: &str = "Reading it\u{2026}";
const TRY_AGAIN: &str = "Try again";
const INSTALL: &str = "Install";
const REPLACE: &str = "Replace";
const RECONFIGURE: &str = "Reconfigure";
const CANCEL: &str = "Cancel";
const CHOOSE: &str = "Choose\u{2026}";
const INSTALL_FAILED: &str = "Could not install it.";
const SCRIPTS_DIR: &str = "~/.look/bin";
const KIND_FOLDER: &str = "folder";
const KIND_FILE: &str = "file";
const KIND_CHOICE: &str = "choice";
const FOLDER_PROMPT: &str = "Use Folder";
const FILE_PROMPT: &str = "Use File";
const FILTER_W: f32 = 160.0;
const SETUP_BOX_W: f32 = 300.0;
const SHEET_GAP: f32 = 10.0;
const FILE_GAP: f32 = 12.0;
const HEADER_GAP: f32 = 6.0;
const ROW_GAP: f32 = 8.0;
const CARD_PADDING: f32 = 10.0;
const CARD_GAP: f32 = 4.0;
const FIELD_GAP: f32 = 2.0;
const FIELDS_GAP: f32 = 8.0;
const FIELDS_PADDING_Y: f32 = 4.0;
const CONTENTS_PADDING: f32 = 8.0;
const CONTENTS_LINE_HEIGHT: f32 = 1.35;
const LINK_UNDERLINE: f32 = 1.0;

#[derive(Clone, Default)]
enum Reading {
    #[default]
    Pending,
    Failed(String),
    Ready(Catalog),
}

/// The catalog as last read, the filter, and the sheet when one is open.
/// Cloned into `Kept` across a hide: a folder pick hides the launcher, and
/// the sheet must still be there for its answer.
#[derive(Clone, Default)]
pub(super) struct Browse {
    reading: Reading,
    /// Programs some entry requires that this machine lacks, probed on
    /// every read: docker may arrive between two visits.
    missing: HashSet<String>,
    /// Bumped by every read, so an older answer cannot land.
    reads: u64,
    /// Likewise for the sheet: the one open is the one answers reach.
    opens: u64,
    /// Mirrored out of the box, so the filter outlives it.
    filter: String,
    /// Installed during this visit, so a card says so before the list
    /// behind it has reloaded.
    installed: HashSet<String>,
    sheet: Option<Sheet>,
}

/// One entry being read before it is written: these are shell commands
/// somebody else wrote, so the sheet shows them and Install is the second
/// click.
#[derive(Clone)]
struct Sheet {
    entry: Entry,
    /// The read that opened it.
    opened: u64,
    /// What it would write over, asked before anything is fetched. One may
    /// be the copy somebody edited, and a notice afterwards cannot give
    /// that back.
    replaces: Vec<String>,
    /// The fetched files, with the answers filled in as they are given.
    files: Option<InstallRequest>,
    /// One per declaration, in the same order.
    previews: Vec<Preview>,
    /// Bumped per answer to a declaration, so an older preview cannot
    /// land over a newer one.
    edits: Vec<u64>,
    failure: Option<String>,
    writing: bool,
}

impl Sheet {
    fn working(&self) -> bool {
        self.files.is_none() && self.failure.is_none()
    }

    fn installable(&self) -> bool {
        self.files.is_some() && self.failure.is_none() && !self.writing
    }

    fn value(&self, file: usize, field: usize) -> String {
        self.files
            .as_ref()
            .and_then(|files| {
                files.declarations[file]
                    .answers
                    .get(field)
                    .cloned()
                    .flatten()
            })
            .or_else(|| self.previews[file].fields[field].current.clone())
            .unwrap_or_default()
    }
}

impl Browse {
    /// A fresh settings session: the catalog is read again, and the sheet
    /// and this visit's installs are gone.
    pub(super) fn forget(&mut self) {
        self.reading = Reading::Pending;
        self.installed.clear();
        self.sheet = None;
        self.reads += 1;
    }

    /// Fetches the catalog when none is in hand, and probes the
    /// requirements either way.
    pub(super) fn read(&mut self, cx: &mut Context<Launcher>) {
        self.reads += 1;
        let read = self.reads;
        let known = match &self.reading {
            Reading::Ready(catalog) => Some(catalog.clone()),
            _ => None,
        };
        bg::fetch(
            cx,
            move || {
                let found = known.map_or_else(lookbook::catalog, Ok);
                let missing = found
                    .as_ref()
                    .map(|catalog| lookbook::missing_programs(&catalog.requires()))
                    .unwrap_or_default();
                (found, missing)
            },
            move |this, (found, missing), _| {
                let browse = &mut this.settings.browse;
                if browse.reads != read {
                    return;
                }
                browse.missing = missing.into_iter().collect();
                browse.reading = match found {
                    Ok(catalog) => Reading::Ready(catalog),
                    Err(message) => Reading::Failed(message),
                };
            },
        );
    }

    fn sheet_mut(&mut self, opened: u64) -> Option<&mut Sheet> {
        self.sheet.as_mut().filter(|sheet| sheet.opened == opened)
    }

    /// `present` first so a replace is named before it happens, then the
    /// files, each previewed for its blocks and `[[setup]]` fields.
    fn open(&mut self, entry: Entry, cx: &mut Context<Launcher>) {
        let Reading::Ready(catalog) = &self.reading else {
            return;
        };
        let catalog = catalog.clone();
        self.opens += 1;
        let opened = self.opens;
        self.sheet = Some(Sheet {
            entry: entry.clone(),
            opened,
            replaces: Vec::new(),
            files: None,
            previews: Vec::new(),
            edits: Vec::new(),
            failure: None,
            writing: false,
        });
        bg::fetch(
            cx,
            move || {
                let present = lookbook::present(&PresentRequest {
                    declarations: entry.files.clone(),
                    scripts: entry.bin.clone(),
                });
                let mut replaces = present
                    .map(|p| [p.declarations, p.scripts].concat())
                    .unwrap_or_default();
                replaces.sort();
                (replaces, read_entry(&catalog, &entry))
            },
            move |this, (replaces, read), _| {
                let Some(sheet) = this.settings.browse.sheet_mut(opened) else {
                    return;
                };
                sheet.replaces = replaces;
                match read {
                    Ok((files, previews)) => {
                        sheet.edits = vec![0; previews.len()];
                        sheet.files = Some(files);
                        sheet.previews = previews;
                    }
                    Err(message) => sheet.failure = Some(message),
                }
            },
        );
    }

    /// One answer, then the declaration previewed again with it filled in:
    /// the readiness lines describe the file as written, and the point of
    /// the form is to stop them being true.
    fn answer(&mut self, file: usize, field: usize, value: String, cx: &mut Context<Launcher>) {
        // A pick lands after a hide and a show, by when the sheet may be
        // another one or gone. While a write runs, the request is already
        // on its way and a change here would be lost with the sheet.
        let Some(sheet) = self.sheet.as_mut().filter(|sheet| !sheet.writing) else {
            return;
        };
        let opened = sheet.opened;
        let Some(current) = sheet
            .previews
            .get(file)
            .and_then(|preview| preview.fields.get(field))
            .map(|setup| setup.current.as_deref())
        else {
            return;
        };
        let Some(files) = sheet.files.as_mut() else {
            return;
        };
        let value = value.trim().to_string();
        let declaration = &mut files.declarations[file];
        // Committing what the box already shows, a click away from it
        // mostly, changes nothing and must not lose the answer.
        let shown = declaration.answers[field].as_deref().or(current);
        if shown.unwrap_or_default() == value {
            return;
        }
        let given = !value.is_empty() && current != Some(value.as_str());
        declaration.answers[field] = given.then_some(value);
        let filled = declaration.clone();
        sheet.edits[file] += 1;
        let edit = sheet.edits[file];
        bg::fetch(
            cx,
            move || lookbook::preview_filled(&filled),
            move |this, found, _| {
                let Some(sheet) = this.settings.browse.sheet_mut(opened) else {
                    return;
                };
                if sheet.edits[file] != edit {
                    return;
                }
                match found {
                    Ok(preview) => sheet.previews[file] = preview,
                    // The form stays up with the answer in it; the line
                    // under the file says why it would not fill.
                    Err(message) => sheet.previews[file].problems = vec![message],
                }
            },
        );
    }

    /// Back from a hide. Work in flight died with the old launcher: a read
    /// runs again, and a write is on disk by now, so the sources reload and
    /// the installed list says what landed.
    pub(super) fn resume(&mut self, cx: &mut Context<Launcher>) {
        let Some(sheet) = self.sheet.take() else {
            return;
        };
        if sheet.writing {
            let launcher = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = launcher.update(cx, |this, cx| this.reload_sources(cx));
            });
        } else if sheet.working() {
            self.open(sheet.entry, cx);
        } else {
            self.sheet = Some(sheet);
        }
    }

    /// Picking beats retyping somebody else's path from memory.
    fn choose(&mut self, file: usize, field: usize, folder: bool, cx: &mut Context<Launcher>) {
        let then =
            move |this: &mut Launcher, path: std::path::PathBuf, cx: &mut Context<Launcher>| {
                this.settings
                    .browse
                    .answer(file, field, path.to_string_lossy().into_owned(), cx);
            };
        if folder {
            pick::folder(FOLDER_PROMPT, cx, then);
        } else {
            pick::file(FILE_PROMPT, cx, then);
        }
    }

    /// Writes, then reloads the sources: the files are on disk and nothing
    /// has read them. Asked whenever anything landed, a failed install too,
    /// since the installer is partial on purpose.
    pub(super) fn install(&mut self, cx: &mut Context<Launcher>) {
        let Some(sheet) = self.sheet.as_mut().filter(|sheet| sheet.installable()) else {
            return;
        };
        let Some(request) = sheet.files.clone() else {
            return;
        };
        sheet.writing = true;
        let opened = sheet.opened;
        let id = sheet.entry.id.clone();
        bg::fetch(
            cx,
            move || lookbook::install(&request, crate::state()),
            move |this, outcome, cx| {
                let Some(sheet) = this.settings.browse.sheet_mut(opened) else {
                    return;
                };
                sheet.writing = false;
                let outcome = match outcome {
                    Ok(outcome) => outcome,
                    Err(_) => {
                        sheet.failure = Some(INSTALL_FAILED.to_string());
                        return;
                    }
                };
                if !outcome.problems.is_empty() {
                    sheet.failure = Some(outcome.problems.join("\n"));
                } else {
                    this.settings.browse.installed.insert(id);
                    this.settings.browse.sheet = None;
                }
                if !outcome.written.is_empty() {
                    this.reload_sources(cx);
                }
            },
        );
    }
}

/// The files and their previews, or why the entry cannot be shown. A
/// preview that came back with problems still installs: part of it parsed,
/// and the sheet shows what did not.
fn read_entry(catalog: &Catalog, entry: &Entry) -> Result<(InstallRequest, Vec<Preview>), String> {
    let mut files = catalog.files(entry)?;
    let mut previews = Vec::with_capacity(files.declarations.len());
    for file in &mut files.declarations {
        let preview = lookbook::preview(&file.contents)
            .map_err(|_| format!("{} is not a declaration Look can read.", file.name))?;
        file.answers = vec![None; preview.fields.len()];
        previews.push(preview);
    }
    Ok((files, previews))
}

/// Filtered in memory: the catalog is already here.
fn matching<'a>(entries: &'a [Entry], filter: &str) -> Vec<&'a Entry> {
    let needle = filter.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| {
            needle.is_empty()
                || entry.name.to_lowercase().contains(&needle)
                || entry.id.contains(&needle)
                || entry.description.to_lowercase().contains(&needle)
        })
        .collect()
}

fn setup_id(verb: &str, file: usize, field: usize) -> ElementId {
    ElementId::Name(format!("lookbook-{verb}-{file}-{field}").into())
}

impl Settings {
    pub(super) fn sheet_open(&self) -> bool {
        self.browse.sheet.is_some()
    }

    pub(super) fn close_sheet(&mut self, cx: &mut Context<Launcher>) {
        self.browse.sheet = None;
        self.field = None;
        self.scroll.set_offset(Default::default());
        cx.notify();
    }

    fn open_sheet(&mut self, entry: Entry, cx: &mut Context<Launcher>) {
        self.field = None;
        self.scroll.set_offset(Default::default());
        self.browse.open(entry, cx);
        cx.notify();
    }

    fn open_filter_field(&mut self, cx: &mut Context<Launcher>) {
        let input = input_for(cx);
        let filter = self.browse.filter.clone();
        input.update(cx, |input, cx| input.set_text(&filter, cx));
        cx.subscribe(&input, |this, input, _: &Changed, cx| {
            this.settings.browse.filter = input.read(cx).text().to_string();
            cx.notify();
        })
        .detach();
        self.field = Some(Field {
            input,
            highlighted: None,
            kind: FieldKind::LookbookFilter,
        });
        cx.notify();
    }

    pub(super) fn close_filter_field(&mut self, cx: &mut Context<Launcher>) {
        if self
            .field
            .as_ref()
            .is_some_and(|f| matches!(f.kind, FieldKind::LookbookFilter))
        {
            self.field = None;
            cx.notify();
        }
    }

    fn open_setup_field(&mut self, file: usize, field: usize, cx: &mut Context<Launcher>) {
        let Some(sheet) = &self.browse.sheet else {
            return;
        };
        let input = input_for(cx);
        let value = sheet.value(file, field);
        input.update(cx, |input, cx| {
            input.set_text(&value, cx);
            input.select_all(cx);
        });
        cx.subscribe(&input, |_, _, _: &Changed, cx| cx.notify())
            .detach();
        self.field = Some(Field {
            input,
            highlighted: None,
            kind: FieldKind::Setup { file, field },
        });
        cx.notify();
    }

    /// Enter, Tab, or a click away: what the box holds.
    pub(super) fn commit_setup(&mut self, file: usize, field: usize, cx: &mut Context<Launcher>) {
        let Some(editing) = self.field.take() else {
            return;
        };
        let typed = editing.input.read(cx).committed();
        self.browse.answer(file, field, typed, cx);
        cx.notify();
    }

    // --- Render ------------------------------------------------------------------

    /// The section under the installed lists. `present` is the file names
    /// in the sources directory, which says what is already here.
    pub(super) fn browse_section(
        &self,
        present: &HashSet<&str>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let ready = matches!(self.browse.reading, Reading::Ready(_));
        let header = div()
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(th.text)
                    .child(TITLE),
            )
            .child(muted(SUBTITLE, th.font_size - 3.0, th))
            .child(div().flex_1())
            .when(ready, |el| el.child(self.filter_box(th, cx)));
        let body: AnyElement = match &self.browse.reading {
            Reading::Pending => muted(READING_CATALOG, th.font_size - 2.0, th).into_any_element(),
            Reading::Failed(message) => div()
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .child(warning(message.clone(), th))
                .child(
                    pill("lookbook-retry".into(), TRY_AGAIN, None, th)
                        .on_click(cx.listener(|this, _, _, cx| this.settings.browse.read(cx))),
                )
                .into_any_element(),
            Reading::Ready(catalog) => {
                let shown = matching(&catalog.entries, &self.browse.filter);
                if shown.is_empty() {
                    muted(
                        format!(
                            "Nothing matches \u{201C}{}\u{201D}.",
                            self.browse.filter.trim()
                        ),
                        th.font_size - 2.0,
                        th,
                    )
                    .into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(ROW_GAP))
                        .children(
                            shown
                                .into_iter()
                                .enumerate()
                                .map(|(i, entry)| self.card(i, entry, catalog, present, th, cx)),
                        )
                        .into_any_element()
                }
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .child(header)
            .child(body)
    }

    fn filter_box(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let editing = self
            .field
            .as_ref()
            .filter(|f| matches!(f.kind, FieldKind::LookbookFilter));
        let filter = &self.browse.filter;
        let content: AnyElement = match editing {
            Some(field) => search_field(field.input.clone(), FILTER_PLACEHOLDER).into_any_element(),
            None => div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .when(filter.is_empty(), |el| el.text_color(th.text_muted))
                .child(if filter.is_empty() {
                    FILTER_PLACEHOLDER.to_string()
                } else {
                    filter.clone()
                })
                .into_any_element(),
        };
        div().flex_shrink_0().child(
            controls::text_box("lookbook-filter", FILTER_W, content, editing.is_some(), th)
                .when(editing.is_none(), |el| {
                    el.on_click(cx.listener(|this, _, _, cx| this.settings.open_filter_field(cx)))
                })
                .when(editing.is_some(), |el| {
                    el.on_mouse_down_out(
                        cx.listener(|this, _, _, cx| this.settings.close_filter_field(cx)),
                    )
                }),
        )
    }

    fn card(
        &self,
        index: usize,
        entry: &Entry,
        catalog: &Catalog,
        present: &HashSet<&str>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let elsewhere = !entry.platforms.iter().any(|p| p == lookbook::THIS_PLATFORM);
        let absent: Vec<&str> = entry
            .requires
            .iter()
            .filter(|program| self.browse.missing.contains(*program))
            .map(String::as_str)
            .collect();
        let here = self.browse.installed.contains(&entry.id)
            || (!entry.files.is_empty()
                && entry
                    .files
                    .iter()
                    .all(|file| present.contains(file.as_str())));
        let header = div()
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .text_size(px(th.font_size - 2.0))
            .child(self.entry_name(index, entry, catalog, th, cx))
            .child(
                div()
                    .font_family(th.mono_family.clone())
                    .text_size(px(th.font_size - 3.0))
                    .text_color(th.text_muted)
                    .child(SharedString::from(entry.id.clone())),
            )
            .child(div().flex_1())
            .map(|el| {
                // Asked first: an entry that cannot run here has no button,
                // whatever is already on disk.
                if elsewhere {
                    el.child(
                        div()
                            .text_color(th.text_muted)
                            .child(SharedString::from(entry.platforms.join(", "))),
                    )
                } else {
                    let opening = entry.clone();
                    el.child(
                        pill(
                            pill_id("lookbook-install", index),
                            if here { RECONFIGURE } else { INSTALL },
                            None,
                            th,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings.open_sheet(opening.clone(), cx);
                        })),
                    )
                }
            });
        div()
            .p(px(CARD_PADDING))
            .rounded(px(th.control_radius()))
            .bg(th.control_fill)
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .child(header)
            .child(
                div()
                    .text_size(px(th.font_size - 3.0))
                    .text_color(th.text_secondary)
                    .child(SharedString::from(entry.description.clone())),
            )
            // Said before installing: the point of a catalog declaring what
            // its helper scripts run.
            .when(!absent.is_empty() && !elsewhere, |el| {
                el.child(
                    div()
                        .text_size(px(th.font_size - 3.0))
                        .text_color(th.warning)
                        .child(SharedString::from(format!(
                            "needs {}, which you do not have",
                            absent.join(", ")
                        ))),
                )
            })
    }

    /// The entry's name, opening its README when the catalog has one: the
    /// question "what does this do" is the one the name stands for.
    fn entry_name(
        &self,
        index: usize,
        entry: &Entry,
        catalog: &Catalog,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> AnyElement {
        let name = SharedString::from(entry.name.clone());
        let Some(readme) = catalog.readme(entry) else {
            return div().text_color(th.text).child(name).into_any_element();
        };
        let accent = crate::theme::hsla_of(th.accent);
        let link = HighlightStyle {
            color: Some(accent),
            underline: Some(UnderlineStyle {
                thickness: px(LINK_UNDERLINE),
                color: Some(accent),
                wavy: false,
            }),
            ..Default::default()
        };
        div()
            .id(pill_id("lookbook-readme", index))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, _| this.open_url(&readme)))
            .child(StyledText::new(name.clone()).with_highlights(vec![(0..name.len(), link)]))
            .into_any_element()
    }

    /// The sheet, in the tab's place: the declaration itself, its fields
    /// above its commands, and the button underneath.
    pub(super) fn sheet(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let Some(sheet) = &self.browse.sheet else {
            return div();
        };
        let body: AnyElement = if sheet.working() {
            muted(READING_ENTRY, th.font_size - 2.0, th).into_any_element()
        } else if let Some(failure) = &sheet.failure {
            warning(failure.clone(), th).into_any_element()
        } else {
            let files = sheet.files.as_ref().expect("files when not working");
            div()
                .flex()
                .flex_col()
                .gap(px(FILE_GAP))
                .children(files.declarations.iter().enumerate().map(|(i, file)| {
                    self.declaration(sheet, i, &file.name, &file.contents, th, cx)
                }))
                .when(!files.scripts.is_empty(), |el| {
                    let names: Vec<&str> = files.scripts.iter().map(|s| s.name.as_str()).collect();
                    el.child(muted(
                        format!("Also installs {} into {SCRIPTS_DIR}.", names.join(", ")),
                        th.font_size - 3.0,
                        th,
                    ))
                })
                .into_any_element()
        };
        let installable = sheet.installable();
        // Named for what it does: the press that takes somebody's edits
        // away should not say Install.
        let verb = if sheet.replaces.is_empty() {
            INSTALL
        } else {
            REPLACE
        };
        div()
            .flex()
            .flex_col()
            .gap(px(SHEET_GAP))
            .child(
                div()
                    .text_size(px(th.font_size + 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(th.text)
                    .child(SharedString::from(sheet.entry.name.clone())),
            )
            .child(
                div()
                    .text_size(px(th.font_size - 2.0))
                    .text_color(th.text_secondary)
                    .child(SharedString::from(sheet.entry.description.clone())),
            )
            .child(body)
            .when(!sheet.replaces.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(px(th.font_size - 3.0))
                        .text_color(th.warning)
                        .child(SharedString::from(format!(
                            "Replaces {}, overwriting any changes you made to it.",
                            sheet.replaces.join(", ")
                        ))),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(ROW_GAP))
                    .child(
                        pill("lookbook-cancel".into(), CANCEL, None, th)
                            .on_click(cx.listener(|this, _, _, cx| this.settings.close_sheet(cx))),
                    )
                    .child(
                        pill(
                            "lookbook-write".into(),
                            verb,
                            (!installable).then_some(th.text_muted),
                            th,
                        )
                        .when(installable, |el| {
                            el.on_click(
                                cx.listener(|this, _, _, cx| this.settings.browse.install(cx)),
                            )
                        }),
                    ),
            )
    }

    fn declaration(
        &self,
        sheet: &Sheet,
        file: usize,
        name: &str,
        contents: &str,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let preview = &sheet.previews[file];
        div()
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .child(
                div()
                    .font_family(th.mono_family.clone())
                    .text_size(px(th.font_size - 2.0))
                    .text_color(th.text_secondary)
                    .child(SharedString::from(name.to_string())),
            )
            // What the example asks for, above the commands: the lines you
            // would otherwise have installed it and gone looking for.
            .when(!preview.fields.is_empty(), |el| {
                el.child(
                    div()
                        .py(px(FIELDS_PADDING_Y))
                        .flex()
                        .flex_col()
                        .gap(px(FIELDS_GAP))
                        .children(
                            preview
                                .fields
                                .iter()
                                .enumerate()
                                .map(|(i, field)| self.control(sheet, file, i, field, th, cx)),
                        ),
                )
            })
            // The same check the Extend screen runs, so this cannot promise
            // something the screen then contradicts. Then the parts that did
            // not parse, which Install would silently leave out.
            .children(
                preview
                    .blocks
                    .iter()
                    .flat_map(|block| {
                        block
                            .needs
                            .iter()
                            .map(|need| format!("{}: {}", block.id, need.message))
                    })
                    .chain(preview.problems.iter().cloned())
                    .map(|line| {
                        div()
                            .text_size(px(th.font_size - 3.0))
                            .text_color(th.warning)
                            .child(SharedString::from(line))
                    }),
            )
            .child(
                div()
                    .p(px(CONTENTS_PADDING))
                    .rounded(px(th.control_radius()))
                    .bg(th.control_fill)
                    .font_family(th.mono_family.clone())
                    .text_size(px(th.font_size - 3.0))
                    .line_height(px((th.font_size - 3.0) * CONTENTS_LINE_HEIGHT))
                    .text_color(th.text)
                    .child(SharedString::from(contents.to_string())),
            )
    }

    /// One field: pills for a choice, a box otherwise, and a picker beside
    /// a path.
    fn control(
        &self,
        sheet: &Sheet,
        file: usize,
        field: usize,
        setup: &Setup,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let value = sheet.value(file, field);
        let control: AnyElement = if setup.kind == KIND_CHOICE {
            div()
                .flex()
                .flex_wrap()
                .gap(px(HEADER_GAP))
                .children(setup.options.iter().enumerate().map(|(i, option)| {
                    let on = *option == value;
                    let picked = option.clone();
                    pill(
                        ElementId::Name(format!("lookbook-choice-{file}-{field}-{i}").into()),
                        SharedString::from(option.clone()),
                        None,
                        th,
                    )
                    .when(on, |el| el.bg(th.accent).text_color(th.on_accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.browse.answer(file, field, picked.clone(), cx);
                    }))
                }))
                .into_any_element()
        } else {
            self.setup_box(file, field, value, th, cx)
                .into_any_element()
        };
        let picks_folder = setup.kind == KIND_FOLDER;
        let picks = picks_folder || setup.kind == KIND_FILE;
        div()
            .flex()
            .flex_col()
            .gap(px(FIELD_GAP))
            .child(
                div()
                    .text_size(px(th.font_size - 2.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(th.text)
                    .child(SharedString::from(setup.label.clone())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(ROW_GAP))
                    .child(control)
                    .when(picks, |el| {
                        el.child(
                            pill(setup_id("choose", file, field), CHOOSE, None, th).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.settings.browse.choose(file, field, picks_folder, cx);
                                }),
                            ),
                        )
                    }),
            )
            .when_some(setup.note.clone(), |el, note| {
                el.child(muted(note, th.font_size - 3.0, th))
            })
    }

    fn setup_box(
        &self,
        file: usize,
        field: usize,
        value: String,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> impl IntoElement {
        let editing = self.field.as_ref().filter(
            |f| matches!(f.kind, FieldKind::Setup { file: f1, field: f2 } if f1 == file && f2 == field),
        );
        let content: AnyElement = match editing {
            Some(editing) => search_field(editing.input.clone(), "").into_any_element(),
            None => div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(th.mono_family.clone())
                .text_size(px(th.font_size - 2.0))
                .child(SharedString::from(value))
                .into_any_element(),
        };
        controls::text_box(
            setup_id("setup", file, field),
            SETUP_BOX_W,
            content,
            editing.is_some(),
            th,
        )
        .when(editing.is_none(), |el| {
            el.on_click(
                cx.listener(move |this, _, _, cx| this.settings.open_setup_field(file, field, cx)),
            )
        })
        .when(editing.is_some(), |el| {
            el.on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                this.settings.commit_setup(file, field, cx);
            }))
        })
    }
}

fn warning(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .text_size(px(th.font_size - 2.0))
        .text_color(th.warning)
        .child(text.into())
}

fn muted(text: impl Into<SharedString>, size: f32, th: &Theme) -> Div {
    div()
        .text_size(px(size))
        .text_color(th.text_muted)
        .child(text.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, description: &str) -> Entry {
        Entry {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            platforms: vec!["linux".into()],
            requires: Vec::new(),
            files: vec![format!("{id}.toml")],
            bin: Vec::new(),
            readme: String::new(),
            path: String::new(),
        }
    }

    #[test]
    fn the_filter_reads_name_id_and_description() {
        let entries = [
            entry("git", "Git", "Branches as rows."),
            entry("tmux", "Tmux", "Sessions to attach to."),
        ];
        assert_eq!(matching(&entries, "").len(), 2);
        assert_eq!(matching(&entries, " GIT ")[0].id, "git");
        assert_eq!(matching(&entries, "attach")[0].id, "tmux");
        assert!(matching(&entries, "docker").is_empty());
    }

    #[test]
    fn the_sheet_shows_the_answer_over_the_authors_value() {
        use linows_backend::look_engine::lookbook::{Field, NamedFile};
        let mut sheet = Sheet {
            entry: entry("notes", "Notes", ""),
            opened: 1,
            replaces: Vec::new(),
            files: Some(InstallRequest {
                declarations: vec![NamedFile {
                    name: "notes.toml".into(),
                    contents: String::new(),
                    answers: vec![None],
                }],
                scripts: Vec::new(),
            }),
            previews: vec![Preview {
                blocks: Vec::new(),
                fields: vec![Field {
                    label: "Vault".into(),
                    kind: KIND_FOLDER,
                    note: None,
                    options: Vec::new(),
                    current: Some("~/vault".into()),
                }],
                problems: Vec::new(),
            }],
            edits: vec![0],
            failure: None,
            writing: false,
        };
        assert_eq!(sheet.value(0, 0), "~/vault");
        sheet.files.as_mut().unwrap().declarations[0].answers[0] = Some("/elsewhere".into());
        assert_eq!(sheet.value(0, 0), "/elsewhere");
        assert!(sheet.installable());
        sheet.writing = true;
        assert!(!sheet.installable());
    }
}
