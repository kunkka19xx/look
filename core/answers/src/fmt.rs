//! Number formatting shared by the answer sources, matching the macOS output:
//! grouped thousands, up to 2 fraction digits (6 for sub-unit values), trailing
//! zeros trimmed.

/// Formats `value` like the macOS decimal formatter: thousands separators, up to
/// 6 fraction digits when `|value| < 1` (for tiny FX/crypto rates) else 2, with
/// trailing zeros removed. E.g. `26252.49`, `0.000038`, `1,234.5`.
pub fn format_number(value: f64) -> String {
    let decimals = if value.abs() < 1.0 { 6 } else { 2 };
    let fixed = format!("{:.*}", decimals, value);
    let trimmed = if fixed.contains('.') {
        fixed
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        fixed
    };
    group_thousands(&trimmed)
}

/// Renders an amount for an API query param: integer form when whole, else the
/// plain decimal (no grouping). E.g. `1`, `2.5`.
pub fn query_amount(value: f64) -> String {
    if value == value.round() {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn group_thousands(s: &str) -> String {
    let negative = s.starts_with('-');
    let body = s.trim_start_matches('-');
    let (int_part, frac_part) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };

    let len = int_part.chars().count();
    let mut grouped = String::with_capacity(len + len / 3);
    for (i, ch) in int_part.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(frac) = frac_part {
        out.push('.');
        out.push_str(frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_trims() {
        assert_eq!(format_number(26252.49), "26,252.49");
        assert_eq!(format_number(1234.0), "1,234");
        assert_eq!(format_number(1234.5), "1,234.5");
    }

    #[test]
    fn small_values_keep_precision() {
        assert_eq!(format_number(0.000038), "0.000038");
    }

    #[test]
    fn query_amount_is_compact() {
        assert_eq!(query_amount(1.0), "1");
        assert_eq!(query_amount(2.5), "2.5");
    }
}
