use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub(crate) fn normalize_for_search(input: &str) -> String {
    // Fast path: pure ASCII avoids Unicode NFKD overhead
    if input.is_ascii() {
        let mut out = input.to_owned();
        out.make_ascii_lowercase();
        return out;
    }

    let mut out = String::with_capacity(input.len());

    for ch in input.nfkd() {
        if is_combining_mark(ch) {
            continue;
        }

        match ch {
            'đ' | 'Đ' => out.push('d'),
            _ => out.extend(ch.to_lowercase()),
        }
    }

    out
}
