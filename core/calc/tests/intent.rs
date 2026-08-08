//! The calculator sees every keystroke typed into the launcher's one search
//! field, so the cost of a false positive is a wrong-looking row on top of a
//! perfectly good search. This corpus is the guard rail: real queries people
//! type at a launcher, and what the gate must decide about each.
//!
//! One rule runs through all of it: shape decides, spacing never does.

use look_calc::is_math;

/// Never evaluated. A result here would be noise on top of a real search.
#[test]
fn launcher_queries_that_are_not_math() {
    let queries = [
        // Dates, the case from issue #325
        "20-05-2026",
        "2026-08-04",
        "10-5-26",
        "3-2-1",
        "2024-01-15 meeting notes",
        // Versions, addresses, encodings
        "1.2.3",
        "v1.2.3",
        "192.168.1.1",
        "utf-8",
        "covid-19",
        // Aliases glued to digits: a resolution, a ratio, a time, a paper size
        "1920x1080",
        "1024x768",
        "8.5x11",
        "16:9",
        "4:3",
        "10:30",
        // Names with numbers in them
        "part-1",
        "chapter-2",
        "IMG_20240115",
        "look 2",
        "todo 2026",
        "10-year plan",
        "p90",
        // Paths
        "~/Documents/2",
        "/usr/2",
        "~/2026",
        // Not calculations even though they parse
        "42",
        "0.5",
        "-5",
        "+7",
        "pi",
        "2 3 4",
        // Half-typed: stay quiet rather than flash an error
        "2 +",
        "sqrt",
        "(2 + 3",
        "",
        "   ",
    ];
    for q in queries {
        assert!(!is_math(q), "{q:?} must not be math");
    }
}

/// Arithmetic, shown without a prefix and without `=?`.
#[test]
fn queries_that_are_math() {
    let queries = [
        "2+2",
        "1+1",
        "100+200",
        "1/1000",
        "2-3",
        "2 - 3",
        "1920 x 1080",
        "10 / 4",
        "2*8",
        "2^10",
        "sqrt(16)",
        "(2+3)*4",
        "50%",
        "5!",
        "1,500 + 1",
        "200 * 15%",
        "2pi",
        "3sqrt(9)",
        "round(3.7)",
        "1e6 / 2",
        "24/7",
        "50/50",
        "1/2",
    ];
    for q in queries {
        assert!(is_math(q), "{q:?} must be math");
    }
}

/// Spacing is a typing habit. Every pair here has to behave the same way.
#[test]
fn spacing_is_never_the_deciding_factor() {
    let pairs = [
        ("1/1000", "1 / 1000"),
        ("2+2", "2 + 2"),
        ("2-3", "2 - 3"),
        ("100*7", "100 * 7"),
        ("2^10", "2 ^ 10"),
        ("50%of200", "50 % of 200"),
    ];
    for (tight, spaced) in pairs {
        assert_eq!(
            is_math(tight),
            is_math(spaced),
            "{tight:?} and {spaced:?} must agree"
        );
    }
}
