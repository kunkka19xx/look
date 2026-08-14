//! Clipboard text operations: turn a short verb the user types ("summarize",
//! "fix grammar", "translate to french") into a model instruction applied to
//! text they already have on the clipboard. A deterministic verb table, so the
//! routing needs no model - the model only does the transformation itself.

pub struct TextOp {
    /// Shown as the user turn in the panel ("Summarize").
    pub label: String,
    /// The system instruction handed to the model, applied to the clipboard.
    pub instruction: String,
}

fn op(label: &str, instruction: &str) -> TextOp {
    TextOp {
        label: label.into(),
        instruction: instruction.into(),
    }
}

/// Parse a bare text-op verb, or None for normal AI input (which falls through
/// to the planner/chat). Only exact verbs match, so "summarize this article
/// about X" stays a chat question, not a clipboard op.
pub fn parse(input: &str) -> Option<TextOp> {
    let lower = input.trim().to_lowercase();

    for prefix in ["translate to ", "translate into ", "translate in "] {
        if let Some(lang) = lower.strip_prefix(prefix) {
            let lang = lang.trim();
            if !lang.is_empty() {
                return Some(op(
                    &format!("Translate to {lang}"),
                    &format!(
                        "Translate the text into {lang}. Output only the translation, no notes."
                    ),
                ));
            }
        }
    }

    Some(match lower.as_str() {
        "summarize" | "summarise" | "summary" | "tldr" | "tl;dr" => op(
            "Summarize",
            "Summarize the text concisely. Output only the summary, nothing else.",
        ),
        "rewrite" | "reword" => op(
            "Rewrite",
            "Rewrite the text to be clearer and smoother, keeping the meaning. Output only the rewrite.",
        ),
        "fix" | "fix grammar" | "proofread" | "grammar" | "correct" => op(
            "Fix grammar",
            "Fix spelling, grammar, and punctuation. Output only the corrected text, preserving meaning and tone.",
        ),
        "shorten" | "shorter" => op(
            "Shorten",
            "Make the text shorter while keeping the meaning. Output only the shortened text.",
        ),
        "expand" | "elaborate" | "longer" => op(
            "Expand",
            "Expand the text with more useful detail. Output only the expanded text.",
        ),
        "formal" | "make formal" | "professional" => op(
            "Make formal",
            "Rewrite the text in a formal, professional tone. Output only the rewrite.",
        ),
        "casual" | "friendly" | "make casual" => op(
            "Make casual",
            "Rewrite the text in a casual, friendly tone. Output only the rewrite.",
        ),
        "bullets" | "bullet" | "bulletize" | "bullet points" => op(
            "Bulletize",
            "Convert the text into concise bullet points. Output only the bullets.",
        ),
        "improve" | "polish" => op(
            "Polish",
            "Improve the writing's clarity, flow, and word choice, keeping the meaning. Output only the improved text.",
        ),
        "explain" | "eli5" => op("Explain", "Explain the text simply and clearly."),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_bare_verbs() {
        assert_eq!(parse("summarize").unwrap().label, "Summarize");
        assert_eq!(parse("Fix Grammar").unwrap().label, "Fix grammar");
        assert_eq!(parse("bullets").unwrap().label, "Bulletize");
    }

    #[test]
    fn translate_captures_language() {
        let t = parse("translate to spanish").unwrap();
        assert_eq!(t.label, "Translate to spanish");
        assert!(t.instruction.contains("spanish"));
        assert!(parse("translate to ").is_none());
    }

    #[test]
    fn normal_input_is_not_a_text_op() {
        assert!(parse("summarize this article about rust").is_none());
        assert!(parse("what's the weather").is_none());
        assert!(parse("add lunch tomorrow").is_none());
    }
}
