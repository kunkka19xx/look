//! `/shell`: one line in, the output or the error out, through the
//! backend's runner with its timeout and output cap.

use gpui::{Context, Div, SharedString, div, prelude::*};
use linows_backend::shell;

use super::Commands;
use crate::bg;
use crate::launcher::Launcher;
use crate::theme::Theme;

pub const ID: &str = "shell";
const PLACEHOLDER: &str = "Type shell command...";
const SELECTED: &str = "Selected /shell";
const RUNNING: &str = "Running...";

#[derive(Default)]
pub struct Shell {
    feedback: String,
    error: bool,
}

impl Shell {
    pub fn enter(&mut self) {
        self.feedback = SELECTED.into();
        self.error = false;
    }

    /// Enter: run `command` off the UI thread; the feedback says so until
    /// it answers.
    pub fn run(&mut self, command: String, cx: &mut Context<Launcher>) {
        if command.is_empty() {
            return;
        }
        self.feedback = RUNNING.into();
        self.error = false;
        bg::fetch(
            cx,
            move || shell::run_shell_command(&command),
            |this, outcome, _| {
                let shell = &mut this.commands.shell;
                match outcome {
                    Ok(output) => {
                        shell.feedback = output;
                        shell.error = false;
                    }
                    Err(err) => {
                        shell.feedback = err;
                        shell.error = true;
                    }
                }
            },
        );
        cx.notify();
    }
}

pub fn panel(frame: &Commands, th: &Theme) -> Div {
    let shell = &frame.shell;
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
                    .id("shell-output")
                    .overflow_y_scroll()
                    .text_color(if shell.error { th.danger } else { th.text })
                    .child(SharedString::from(shell.feedback.clone())),
            ),
        )
}
