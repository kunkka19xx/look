//! The inline web answer card pinned over the results: a port of the macOS
//! `AIAnswerController`. Same triggers, same 350 ms debounce, same source
//! fan-out and dedup. linows has no on-device model, so DuckDuckGo,
//! Wikipedia and the pattern-gated instant providers are the whole of it.

use std::time::Duration;

use gpui::{Context, Div, FontWeight, SharedString, div, prelude::*, px, svg};
use linows_backend::answers;
use linows_backend::look_answers::Answer;

use crate::bg;
use crate::glyphs;
use crate::launcher::Launcher;
use crate::theme::{self, Theme};

const DEBOUNCE: Duration = Duration::from_millis(350);
/// Two answers are the same when their leading characters match: DuckDuckGo
/// abstracts are often verbatim Wikipedia.
const SIMILARITY_PREFIX: usize = 60;
const PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const BLOCK_GAP: f32 = 14.0;
const ICON: f32 = 16.0;
const LINE_HEIGHT: f32 = 1.5;
const THINKING: &str = "Thinking\u{2026}";
const NO_ANSWER: &str = "Couldn't find an answer.";
const FALLBACK_TITLE: &str = "Web answer";
pub const AI_KEY: &str = "ai_enabled";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idle,
    Streaming,
    Done,
    Failed,
}

#[derive(Default)]
pub struct AiAnswer {
    state: Option<State>,
    question: String,
    items: Vec<Answer>,
    /// Bumps on every update; a run that comes back for an older one is dropped.
    version: u64,
    pub enabled: bool,
}

/// "This looks like a question, not an app launch."
pub fn is_question_like(text: &str) -> bool {
    const STARTERS: [&str; 29] = [
        "how",
        "what",
        "why",
        "who",
        "when",
        "where",
        "which",
        "whose",
        "can",
        "could",
        "should",
        "would",
        "is",
        "are",
        "am",
        "do",
        "does",
        "did",
        "will",
        "explain",
        "tell",
        "give",
        "write",
        "summarize",
        "summarise",
        "define",
        "translate",
        "convert",
        "calculate",
    ];
    if text.len() < 3 {
        return false;
    }
    if text.ends_with('?') {
        return true;
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    words.len() >= 3 && STARTERS.contains(&words[0].to_lowercase().as_str())
}

/// Multi-word and long enough to be a name rather than a half-typed token.
pub fn is_entity_lookup(text: &str) -> bool {
    text.len() >= 5 && text.split_whitespace().count() >= 2
}

fn similar(a: &str, b: &str) -> bool {
    let key = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(SIMILARITY_PREFIX)
            .collect::<String>()
    };
    key(a) == key(b)
}

impl AiAnswer {
    pub fn state(&self) -> State {
        self.state.unwrap_or(State::Idle)
    }

    pub fn is_active(&self) -> bool {
        self.state() != State::Idle
    }

    /// Re-evaluate for the query once the local rows are in. `local_count`
    /// is how many the engine found: a multi-word entity with none is a
    /// knowledge lookup. The gates are local, so this runs per keystroke.
    pub fn update(&mut self, query: &str, local_count: usize, cx: &mut Context<Launcher>) {
        let query = query.trim().to_string();
        if !self.enabled || query.is_empty() {
            self.cancel();
            return;
        }
        let question_like = is_question_like(&query);
        let orphan = local_count == 0 && is_entity_lookup(&query);
        // Arithmetic has its own row; a numeric query would read as an
        // orphan entity and send a stray lookup.
        if linows_backend::calc::calc_inline(&query).is_some() {
            self.cancel();
            return;
        }
        let instant = answers::instant_has_match(&query);
        if !(question_like || orphan || instant) {
            self.cancel();
            return;
        }
        if query == self.question && self.is_active() {
            return;
        }
        self.version += 1;
        let version = self.version;
        self.question = query.clone();
        self.items.clear();
        self.state = Some(State::Streaming);

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            // One source at a time lands as it answers; a matched instant
            // provider is what the user wants, so the encyclopedias wait.
            let fetches: Vec<Box<dyn FnOnce() -> Option<Answer> + Send>> = if instant {
                let q = query.clone();
                vec![Box::new(move || answers::instant_answer(&q))]
            } else {
                let term = if question_like {
                    answers::definitional_entity(&query)
                } else {
                    answers::definitional_entity(&query).or_else(|| Some(query.clone()))
                };
                let q = query.clone();
                let mut list: Vec<Box<dyn FnOnce() -> Option<Answer> + Send>> =
                    vec![Box::new(move || answers::duckduckgo_answer(&q))];
                if let Some(term) = term {
                    list.push(Box::new(move || answers::wikipedia_answer(&term)));
                }
                list
            };
            let mut tasks = Vec::new();
            for fetch in fetches {
                tasks.push(cx.background_executor().spawn(async move { fetch() }));
            }
            for task in tasks {
                let answer = task.await;
                let _ = this.update(cx, |this, cx| {
                    if this.ai.version == version {
                        this.ai.land(answer);
                        cx.notify();
                    }
                });
            }
            // An instant provider that came back empty means its API failed,
            // not that there is nothing to say.
            let empty = this
                .update(cx, |this, _| {
                    this.ai.version == version && this.ai.items.is_empty()
                })
                .unwrap_or(false);
            if instant && empty {
                let q1 = query.clone();
                let q2 = query.clone();
                let ddg = cx
                    .background_executor()
                    .spawn(async move { answers::duckduckgo_answer(&q1) });
                let wiki = cx
                    .background_executor()
                    .spawn(async move { answers::wikipedia_answer(&q2) });
                for task in [ddg, wiki] {
                    let answer = task.await;
                    let _ = this.update(cx, |this, cx| {
                        if this.ai.version == version {
                            this.ai.land(answer);
                            cx.notify();
                        }
                    });
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.ai.version == version {
                    this.ai.state = Some(if this.ai.items.is_empty() {
                        State::Failed
                    } else {
                        State::Done
                    });
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn land(&mut self, answer: Option<Answer>) {
        let Some(answer) = answer else {
            return;
        };
        if self.items.iter().any(|it| it.source == answer.source)
            || self.items.iter().any(|it| similar(&it.text, &answer.text))
        {
            return;
        }
        self.items.push(answer);
    }

    /// Tear the card down: query cleared, mode switched, AI off.
    pub fn cancel(&mut self) {
        self.version += 1;
        self.state = None;
        self.question.clear();
        self.items.clear();
    }

    /// The card: header with the question, one block per source, a status
    /// line while nothing has landed.
    pub fn render(&self, th: &Theme, cx: &mut Context<Launcher>) -> Option<Div> {
        let state = self.state?;
        let blocks = self.items.iter().enumerate().map(|(i, item)| {
            let url = item.url.clone();
            let source = div()
                .id(("ai-source", i))
                .flex()
                .items_center()
                .gap(px(theme::CHIP_PADDING_X / 2.0))
                .text_size(px(th.font_size - 4.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if url.is_some() {
                    th.accent
                } else {
                    th.text_muted
                })
                .when(url.is_some(), |el| el.cursor_pointer())
                .on_click(cx.listener(move |this, _, _, _| {
                    if let Some(url) = &url {
                        this.open_url(url);
                    }
                }))
                .child(SharedString::from(item.source.to_uppercase()))
                .when(item.url.is_some(), |el| {
                    el.child(
                        svg()
                            .path(glyphs::ARROW_UP_RIGHT)
                            .size(px(th.font_size - 3.0))
                            .text_color(th.accent),
                    )
                });
            let text = item.text.clone();
            let copy = div()
                .id(("ai-copy", i))
                .p(px(theme::CHIP_PADDING_X / 2.0))
                .rounded(px(th.chip_radius()))
                .text_color(th.text_muted)
                .cursor_pointer()
                .hover(|s| s.bg(th.control_fill).text_color(th.text))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let text = text.clone();
                    bg::fetch(
                        cx,
                        move || linows_backend::clipboard::copy_to_clipboard(&text),
                        |this, outcome, cx| {
                            let (msg, tone) = match outcome {
                                Ok(()) => ("Copied answer", crate::launchpad::Tone::Success),
                                Err(_) => ("Copy failed", crate::launchpad::Tone::Error),
                            };
                            this.banner.show(msg.into(), tone, crate::banner::SHORT, cx);
                        },
                    );
                    let _ = this;
                }))
                .child(
                    svg()
                        .path(glyphs::COPY)
                        .size(px(th.font_size - 1.0))
                        .text_color(th.text_muted),
                );
            div()
                .flex()
                .flex_col()
                .gap(px(GAP / 2.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(source)
                        .child(copy),
                )
                .child(
                    div()
                        .text_size(px(th.font_size - 1.0))
                        .line_height(px((th.font_size - 1.0) * LINE_HEIGHT))
                        .text_color(th.text_secondary)
                        .child(item.text.clone()),
                )
        });
        let status = match (state, self.items.is_empty()) {
            (State::Streaming, true) => Some(THINKING),
            (State::Failed, true) => Some(NO_ANSWER),
            _ => None,
        };
        let title = if self.question.is_empty() {
            FALLBACK_TITLE.to_string()
        } else {
            self.question.clone()
        };
        Some(
            div()
                .mb(px(GAP))
                .p(px(PADDING))
                .rounded(px(th.bar_radius()))
                .bg(th.control_fill)
                .border(px(1.0))
                .border_color(th.border)
                .flex()
                .flex_col()
                .gap(px(GAP))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(GAP - 2.0))
                        .child(
                            svg()
                                .path(glyphs::SPARKLES)
                                .size(px(ICON))
                                .text_color(th.accent),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(title),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(BLOCK_GAP))
                        .children(blocks)
                        .when_some(status, |el, status| {
                            el.child(
                                div()
                                    .text_size(px(th.font_size - 1.0))
                                    .text_color(th.text_muted)
                                    .child(status),
                            )
                        }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn questions_and_entities_are_told_apart() {
        assert!(is_question_like("what is rust?"));
        assert!(is_question_like("how do i exit vim"));
        assert!(!is_question_like("spotify"));
        assert!(!is_question_like("is it"));
        assert!(is_entity_lookup("david beckham"));
        assert!(!is_entity_lookup("vim"));
    }

    #[test]
    fn near_identical_texts_are_similar() {
        assert!(similar("Rust is a language.", "rust IS a language."));
        assert!(!similar("Rust", "Go"));
    }
}
