use look_emoji::{Emoji, search, search_in};

const DATA: &[Emoji] = &[
    Emoji {
        glyph: "😀",
        name: "grinning face",
        keywords: &["grin", "happy"],
    },
    Emoji {
        glyph: "😄",
        name: "smiling face with open mouth",
        keywords: &["smile", "joy"],
    },
    Emoji {
        glyph: "🙂",
        name: "slightly smiling face",
        keywords: &["fine"],
    },
    Emoji {
        glyph: "😊",
        name: "blushing face",
        keywords: &["smile", "blush"],
    },
    Emoji {
        glyph: "🔥",
        name: "fire",
        keywords: &["flame", "hot"],
    },
];

fn glyphs(q: &str, limit: usize) -> Vec<String> {
    search_in(DATA, q, limit)
        .into_iter()
        .map(|h| h.glyph)
        .collect()
}

#[test]
fn matches_by_name() {
    assert_eq!(glyphs("fire", 10), ["🔥"]);
}

#[test]
fn matches_by_keyword_not_in_name() {
    assert_eq!(glyphs("flame", 10), ["🔥"]);
}

#[test]
fn ignores_case_and_surrounding_whitespace() {
    assert_eq!(glyphs("  FIRE ", 10), glyphs("fire", 10));
    assert!(!glyphs("fire", 10).is_empty());
}

#[test]
fn name_prefix_ranks_before_name_substring_before_keyword() {
    // "smil": prefix of "smiling face..." (😄), substring of "slightly smiling"
    // (🙂), keyword-only never matches here ("smile" contains "smil" -> 😊).
    assert_eq!(glyphs("smil", 10), ["😄", "🙂", "😊"]);
}

#[test]
fn ties_keep_dataset_order() {
    assert_eq!(glyphs("face", 10), ["😀", "😄", "🙂", "😊"]);
}

#[test]
fn honours_limit() {
    assert_eq!(glyphs("face", 2).len(), 2);
    assert!(glyphs("face", 0).is_empty());
}

#[test]
fn empty_or_blank_query_returns_nothing() {
    assert!(glyphs("", 10).is_empty());
    assert!(glyphs("   ", 10).is_empty());
}

#[test]
fn no_match_returns_nothing() {
    assert!(glyphs("zzzz", 10).is_empty());
}

#[test]
fn hit_carries_glyph_and_name() {
    let hits = search_in(DATA, "fire", 1);
    assert_eq!(
        (hits[0].glyph.as_str(), hits[0].name.as_str()),
        ("🔥", "fire")
    );
}

#[test]
fn bundled_search_finds_common_emoji() {
    assert!(search("smile", 10).iter().any(|h| !h.glyph.is_empty()));
}

#[test]
fn lookup_resolves_known_and_rejects_unknown_glyphs() {
    assert!(look_emoji::lookup("🔥").is_some());
    assert!(look_emoji::lookup("not an emoji").is_none());
}

#[test]
fn full_dataset_query_is_within_latency_budget() {
    let start = std::time::Instant::now();
    let _ = search("face", 60);
    assert!(start.elapsed() < std::time::Duration::from_millis(20));
}
