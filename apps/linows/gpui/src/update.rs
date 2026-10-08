//! Version and update status, the macOS `AppUpdateStatusView`: shown at the
//! foot of Settings > Advanced as About and under the help screen's title,
//! one state for both. The check runs in the backend off the UI thread;
//! Windows installer builds get an Update button, package installs are
//! told where the command is.

use gpui::prelude::*;
use gpui::{ClipboardItem, Context, Div, FontWeight, SharedString, Stateful, div, px};
use linows_backend::update::{self, Check, Release};
use linows_backend::{files, launch};

use crate::bg;
use crate::controls;
use crate::launcher::Launcher;
use crate::theme::Theme;

const VERSION: &str = env!("LOOK_VERSION");
const DEV_SUFFIX: &str = " - dev";
const CHECK: &str = "Check for Updates";
const CHECKING: &str = "Checking\u{2026}";
const UPDATE: &str = "Update";
const NOTES: &str = "Release Notes";
const DISMISS: &str = "Dismiss";
const COPY: &str = "Copy";
const SEE_INSTRUCTIONS: &str = "see install instructions";
const CHECK_FAILED: &str = "Couldn't check for updates";
const DOWNLOADING: &str = "Downloading update\u{2026}";
const COPIED: &str = "Command copied";
const NSIS_HINT: &str = "Update downloads the installer first, then Look restarts to install it.";
const OS_LABEL: &str = if cfg!(windows) { "Windows" } else { "Linux" };

const ROW_GAP: f32 = 8.0;
const BLOCK_GAP: f32 = 6.0;
const PILL_MUTED_ALPHA: f32 = 0.7;

pub struct Update {
    install_method: String,
    available: Option<Release>,
    status: String,
    checking: bool,
    updating: bool,
}

impl Update {
    pub fn new() -> Self {
        Self {
            install_method: launch::get_install_method(),
            available: None,
            status: String::new(),
            checking: false,
            updating: false,
        }
    }

    fn version_label() -> String {
        let dev = if files::is_dev_build() {
            DEV_SUFFIX
        } else {
            ""
        };
        format!("Look {VERSION}{dev}")
    }

    fn self_updates(&self) -> bool {
        cfg!(windows)
            && !files::is_dev_build()
            && self.install_method == update::SELF_UPDATE_INSTALL_METHOD
    }

    fn busy(&self) -> bool {
        self.checking || self.updating
    }

    // --- Actions -----------------------------------------------------------------

    fn check(&mut self, cx: &mut Context<Launcher>) {
        if self.busy() {
            return;
        }
        self.checking = true;
        self.status = CHECKING.to_string();
        bg::fetch(
            cx,
            || update::check(VERSION, true),
            |this, result, _| {
                let u = &mut this.update;
                u.checking = false;
                match result {
                    Check::Available(release) => {
                        u.available = Some(release);
                        u.status.clear();
                    }
                    Check::Latest => {
                        u.available = None;
                        u.status = format!("You're on the latest version ({VERSION})");
                    }
                    Check::Dismissed => {
                        u.available = None;
                        u.status.clear();
                    }
                    Check::Failed => {
                        u.available = None;
                        u.status = CHECK_FAILED.to_string();
                    }
                }
            },
        );
        cx.notify();
    }

    fn dismiss(&mut self, cx: &mut Context<Launcher>) {
        if let Some(release) = self.available.take() {
            let version = release.version;
            cx.background_executor()
                .spawn(async move { update::dismiss(&version) })
                .detach();
        }
        self.status.clear();
        cx.notify();
    }

    fn copy_command(&mut self, cx: &mut Context<Launcher>) {
        cx.write_to_clipboard(ClipboardItem::new_string(
            update::SCOOP_UPDATE_COMMAND.to_string(),
        ));
        self.status = COPIED.to_string();
        cx.notify();
    }

    /// Windows installer builds: download, then the process exits so the
    /// installer can replace it.
    fn start_update(&mut self, cx: &mut Context<Launcher>) {
        let Some(release) = self.available.clone() else {
            return;
        };
        if !self.self_updates() || self.busy() {
            return;
        }
        self.updating = true;
        self.status = DOWNLOADING.to_string();
        bg::fetch(
            cx,
            move || launch::start_windows_update(&release.version),
            |this, result, _| match result {
                Ok(()) => this.quit(),
                Err(err) => {
                    this.update.updating = false;
                    this.update.status = err;
                }
            },
        );
        cx.notify();
    }

    // --- Render ------------------------------------------------------------------

    /// The status after the version, when no banner carries it.
    fn quiet_status(&self) -> Option<String> {
        (!self.status.is_empty() && self.available.is_none()).then(|| self.status.clone())
    }

    fn check_button(&self, th: &Theme, cx: &mut Context<Launcher>) -> Stateful<Div> {
        pill(
            "update-check",
            if self.checking { CHECKING } else { CHECK },
            false,
            th,
        )
        .when(!self.busy(), |el| {
            el.on_click(cx.listener(|this, _, _, cx| this.update.check(cx)))
        })
    }

    /// The help header's block: version and status, the check button at
    /// the row's end, the banner under it.
    pub fn view(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let row = div()
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(Self::version_label()),
            )
            .children(self.quiet_status().map(|s| muted(s, th)))
            .child(div().flex_1())
            .child(self.check_button(th, cx));
        div()
            .flex()
            .flex_col()
            .gap(px(BLOCK_GAP))
            .child(row)
            .children(
                self.available
                    .as_ref()
                    .map(|release| self.banner(release, th, cx)),
            )
    }

    /// Settings > About as settings rows: the version in the label column,
    /// the check button in the control column, the status as its hint, the
    /// banner on rows of its own under them.
    pub fn about(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(
                controls::row(Self::version_label(), th)
                    .child(self.check_button(th, cx))
                    .children(self.quiet_status().map(|s| controls::hint(s, th))),
            )
            .children(
                self.available
                    .as_ref()
                    .map(|release| controls::row("", th).child(self.banner(release, th, cx))),
            )
    }

    fn banner(&self, release: &Release, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let busy = self.busy();
        let url = release.url.clone();
        let row = div()
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Update available: Look {}", release.version)),
            )
            .when(self.self_updates(), |el| {
                el.child(pill("update-run", UPDATE, false, th).when(!busy, |el| {
                    el.on_click(cx.listener(|this, _, _, cx| this.update.start_update(cx)))
                }))
            })
            .child(
                pill("update-notes", NOTES, false, th)
                    .on_click(cx.listener(move |this, _, _, _| this.open_url(&url))),
            )
            .child(
                pill("update-dismiss", DISMISS, true, th)
                    .on_click(cx.listener(|this, _, _, cx| this.update.dismiss(cx))),
            );
        let hint: Div = if cfg!(windows) && self.install_method == update::SCOOP_INSTALL_METHOD {
            div()
                .flex()
                .items_center()
                .gap(px(ROW_GAP))
                .child(muted(
                    format!(
                        "Quit Look, then run {} once Scoop has the new version.",
                        update::SCOOP_UPDATE_COMMAND
                    ),
                    th,
                ))
                .child(
                    pill("update-copy", COPY, true, th)
                        .on_click(cx.listener(|this, _, _, cx| this.update.copy_command(cx))),
                )
        } else if self.self_updates() {
            muted(NSIS_HINT, th)
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(ROW_GAP / 2.0))
                .child(muted(format!("Update on {OS_LABEL}:"), th))
                .child(
                    div()
                        .id("update-hint")
                        .text_size(px(th.font_size - 2.0))
                        .text_color(th.accent)
                        .cursor_pointer()
                        .hover(|s| s.text_color(th.text))
                        .on_click(
                            cx.listener(|this, _, _, _| this.open_url(update::INSTALL_HINT_URL)),
                        )
                        .child(SEE_INSTRUCTIONS),
                )
        };
        let status = (!self.status.is_empty()).then(|| muted(self.status.clone(), th));
        div()
            .flex()
            .flex_col()
            .gap(px(BLOCK_GAP))
            .child(row)
            .child(hint)
            .children(status)
    }
}

fn muted(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .text_size(px(th.font_size - 2.0))
        .text_color(th.text_muted)
        .child(text.into())
}

/// The app's button, dimmed for the secondary ones.
fn pill(id: &'static str, label: &'static str, quiet: bool, th: &Theme) -> Stateful<Div> {
    controls::button(id, label, th).when(quiet, |el| el.opacity(PILL_MUTED_ALPHA))
}
