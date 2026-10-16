//! `/calc`: the expression evaluates as it is typed, and Enter copies the
//! result. Core's evaluator is the same one behind the inline calc row.

use gpui::{Context, Div, SharedString, div, prelude::*};
use linows_backend::{calc, clipboard};

use super::Commands;
use crate::banner;
use crate::bg;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::theme::Theme;

pub const ID: &str = "calc";
const PLACEHOLDER: &str = "Type math expression";
const SELECTED: &str = "Selected /calc";
const INVALID: &str = "Invalid expression";
const COPIED: &str = "Result copied";

#[derive(Default)]
pub struct Calc {
    feedback: String,
    error: bool,
}

impl Calc {
    pub fn enter(&mut self) {
        self.feedback = SELECTED.into();
        self.error = false;
    }

    /// The box changed: a result replaces the feedback, an error leaves the
    /// last one standing, as the webview's live preview does.
    pub fn preview(&mut self, expr: &str) {
        if expr.is_empty() {
            self.enter();
            return;
        }
        if let Ok(result) = calc::eval_calc(expr) {
            self.feedback = result;
            self.error = false;
        }
    }

    /// Enter: the result, said and copied; an error, said in the danger colour.
    pub fn run(&mut self, expr: &str, cx: &mut Context<Launcher>) {
        if expr.is_empty() {
            return;
        }
        match calc::eval_calc(expr) {
            Ok(result) => {
                self.feedback = result.clone();
                self.error = false;
                bg::fetch(
                    cx,
                    move || clipboard::copy_to_clipboard(&result),
                    |this, outcome, cx| {
                        let (text, tone) = match outcome {
                            Ok(()) => (COPIED, Tone::Success),
                            Err(_) => ("Copy failed", Tone::Error),
                        };
                        this.banner.show(text.into(), tone, banner::SHORT, cx);
                    },
                );
            }
            Err(err) => {
                self.feedback = if err.is_empty() { INVALID.into() } else { err };
                self.error = true;
            }
        }
        cx.notify();
    }
}

pub fn panel(frame: &Commands, th: &Theme) -> Div {
    let calc = &frame.calc;
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(frame.input_bar(PLACEHOLDER, th))
        .child(
            frame.content().child(
                frame
                    .feedback(th)
                    .text_color(if calc.error { th.danger } else { th.text })
                    .child(SharedString::from(calc.feedback.clone())),
            ),
        )
}
