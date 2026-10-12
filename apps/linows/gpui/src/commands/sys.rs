//! `/sys`: a read-only table of what the backend collects about the
//! machine, in sections, loaded when the panel opens.

use gpui::{Context, Div, FontWeight, div, prelude::*, px};
use linows_backend::sysinfo::{self, SysInfoEntry};

use super::Commands;
use crate::bg;
use crate::launcher::Launcher;
use crate::theme::Theme;

pub const ID: &str = "sys";
const SUBTITLE: &str = "Read-only command";
const LOADING: &str = "Loading...";
const NO_DATA: &str = "No data";
const LABEL_W: f32 = 80.0;
const ROW_PADDING_Y: f32 = 2.0;
const SECTION_GAP: f32 = 14.0;

#[derive(Default)]
pub struct Sys {
    sections: Vec<Vec<SysInfoEntry>>,
    loaded: bool,
}

impl Sys {
    pub fn enter(&mut self, cx: &mut Context<Launcher>) {
        self.loaded = false;
        bg::fetch(cx, sysinfo::get_system_info, |this, sections, _| {
            this.commands.sys.sections = sections;
            this.commands.sys.loaded = true;
        });
    }
}

pub fn panel(frame: &Commands, th: &Theme) -> Div {
    let sys = &frame.sys;
    let feedback = match (sys.loaded, sys.sections.is_empty()) {
        (false, _) => Some(LOADING),
        (true, true) => Some(NO_DATA),
        (true, false) => None,
    };
    let rows = sys.sections.iter().enumerate().flat_map(|(si, section)| {
        let spacer = (si > 0).then(|| div().h(px(SECTION_GAP)));
        spacer.into_iter().chain(section.iter().map(|entry| {
            div()
                .py(px(ROW_PADDING_Y))
                .flex()
                .items_baseline()
                .child(
                    div()
                        .w(px(LABEL_W))
                        .flex_shrink_0()
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_secondary)
                        .child(entry.label.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(th.mono_family.clone())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(entry.value.clone()),
                )
        }))
    });
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            frame
                .bar(th)
                .child(div().flex_1().text_color(th.text_secondary).child(SUBTITLE))
                .child(frame.pill(th)),
        )
        .child(
            frame
                .content()
                .when_some(feedback, |el, text| {
                    el.child(frame.feedback(th).child(text))
                })
                .child(
                    div()
                        .id("sys-table")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .children(rows),
                ),
        )
}
