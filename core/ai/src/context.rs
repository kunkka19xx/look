//! Context windowing: how much recent conversation the model sees. A fixed
//! item count is blind to size (10 code-heavy turns overflow a small model; 10
//! one-liners waste the budget), so we keep the newest turns that fit a token
//! budget instead. Model-free - a chars/4 estimate, the standard heuristic.

/// Default history budget in tokens. Sized to keep a local 7B model fast: prompt
/// processing time grows with context, so history stays a few thousand tokens.
pub const DEFAULT_BUDGET_TOKENS: usize = 2500;

/// Rough token estimate for budgeting, plus a small per-message overhead for the
/// role/formatting the transport adds.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count() / 4 + 4
}

/// How many of the most-recent items fit within `budget_tokens`. Always keeps at
/// least the last item, so a single oversized turn is never dropped whole.
pub fn window_count(item_texts: &[String], budget_tokens: usize) -> usize {
    let mut running = 0usize;
    let mut count = 0usize;
    for text in item_texts.iter().rev() {
        let est = estimate_tokens(text);
        if count > 0 && running + est > budget_tokens {
            break;
        }
        running += est;
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lens: &[usize]) -> Vec<String> {
        lens.iter().map(|&n| "x".repeat(n)).collect()
    }

    #[test]
    fn keeps_the_newest_that_fit() {
        // Each ~250 tokens (1000 chars / 4). Budget 600 -> only the last 2 fit.
        let items = texts(&[1000, 1000, 1000, 1000]);
        assert_eq!(window_count(&items, 600), 2);
    }

    #[test]
    fn short_history_fits_entirely() {
        let items = texts(&[40, 40, 40]);
        assert_eq!(window_count(&items, DEFAULT_BUDGET_TOKENS), 3);
    }

    #[test]
    fn always_keeps_at_least_the_last_item() {
        // One huge turn that alone blows the budget is still kept.
        let items = texts(&[100_000]);
        assert_eq!(window_count(&items, 100), 1);
    }

    #[test]
    fn empty_history_is_zero() {
        assert_eq!(window_count(&[], DEFAULT_BUDGET_TOKENS), 0);
    }
}
