//! Expression evaluation for the launcher's calculator - the shared source of
//! truth every shell reads (linows over IPC, macOS over the FFI bridge).
//!
//! Two halves, and the second one matters as much as the first:
//!
//! - [`eval`] parses and evaluates. It answers or it errors; it never guesses.
//! - [`is_math`] decides whether a query was *meant* as arithmetic at all.
//!   A launcher evaluates whatever you type into the one search field, so a
//!   folder named `20-05-2026` must not come back as `-2011`. It rules a query
//!   out by shape, never by spacing: `1/1000` and `1 / 1000` are one thought
//!   typed two ways and always behave the same.

use serde::{Deserialize, Serialize};
use std::f64::consts;

/// Factorials above this overflow f64.
const MAX_FACTORIAL: u64 = 170;

/// Past these magnitudes plain decimal notation stops being readable, so the
/// result is formatted in scientific notation instead.
const SCI_UPPER: f64 = 1e15;
const SCI_LOWER: f64 = 1e-10;

/// Decimal places kept before trailing zeros are trimmed. Ten is wide enough
/// to show a real value like `1/100000` and narrow enough to swallow binary
/// float noise (`0.1 + 0.2` prints as `0.3`, not `0.30000000000000004`).
const DECIMALS: usize = 10;

/// Queries longer than this are prose, not arithmetic.
const MAX_QUERY_LEN: usize = 200;

/// A successful evaluation, in both the shape a human reads and the shape a
/// machine accepts. The clipboard gets `raw`: pasting `1,000,000` into a
/// spreadsheet or a shell is a bug, not a courtesy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calculation {
    /// Grouped for display: `1,000,000`.
    pub display: String,
    /// Bare, and re-parseable by [`eval`]: `1000000`.
    pub raw: String,
    pub value: f64,
}

/// Whether the alias operators that double as ordinary characters (`x`, `:`)
/// are honoured wherever they appear, or only when they stand alone.
///
/// [`eval`] is [`Aliases::Free`]: reaching the evaluator means the caller
/// already knows arithmetic was intended. [`is_math`] is [`Aliases::Standalone`],
/// because it has to tell `1920x1080` and `16:9` apart from a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aliases {
    Free,
    Standalone,
}

/// Evaluate an expression. Returns the formatted result or a human-readable
/// error.
pub fn eval(expr: &str) -> Result<Calculation, String> {
    let value = eval_value(expr)?;
    Ok(Calculation {
        display: format_display(value),
        raw: format_raw(value),
        value,
    })
}

/// The whole inline path in one call: is this query arithmetic, and if so what
/// does it come to? `None` means "leave the search alone".
///
/// This is what a launcher's main field should call on every keystroke. It is
/// pure arithmetic on a short string, so it costs nothing and needs no debounce.
pub fn eval_query(query: &str) -> Option<Calculation> {
    let expr = normalize(query);
    if !is_math(expr) {
        return None;
    }
    eval(expr).ok()
}

/// Strip the punctuation people tack onto arithmetic when they expect an
/// answer back: `2+2=`, `2+2?`, `2+2 =?`.
pub fn normalize(query: &str) -> &str {
    query.trim().trim_end_matches(['=', '?', ' ', '\t'])
}

/// Evaluate to a bare `f64`, for callers that do their own formatting.
pub fn eval_value(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr, Aliases::Free)?;
    if tokens.is_empty() {
        return Err("Empty expression".into());
    }
    let mut pos = 0;
    let result = parse_add_sub(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(format!("Unexpected token: {:?}", tokens[pos]));
    }
    if result.is_nan() {
        return Err("Result is undefined".into());
    }
    if result.is_infinite() {
        return Err("Result is too large to represent".into());
    }
    Ok(result)
}

/// Was this query meant as arithmetic?
///
/// Only the *shape* of the query decides. Whitespace never does: `1/1000` and
/// `1 / 1000` are the same thought typed by two people, and a launcher that
/// answers one but not the other is just guessing at habits. What rules a
/// query out is looking like something else - a date, a version, an address, a
/// path, a name with digits in it.
pub fn is_math(query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() || q.len() > MAX_QUERY_LEN {
        return false;
    }
    // A leading `/` or `~` is a path being typed, even when the rest is digits.
    if q.starts_with('/') || q.starts_with('~') {
        return false;
    }
    // `20-05-2026`, `2026-08-04`, `1.2.3`, `192.168.1.1`: three or more numeric
    // groups joined by separators that double as operators. People name files
    // and folders this way constantly; nobody writes arithmetic this way.
    if is_grouped_numeric(q) {
        return false;
    }
    // Anything the tokenizer rejects (unknown words, stray characters, an
    // alias glued to digits) was never arithmetic: `look 2`, `part-1`,
    // `1920x1080`, `~/Documents`.
    let Ok(tokens) = tokenize(q, Aliases::Standalone) else {
        return false;
    };
    if tokens.is_empty() || !tokens.iter().any(is_operand) {
        return false;
    }
    // A bare number is not a calculation, and a dangling operator is a
    // half-typed one - stay quiet instead of flashing an error mid-keystroke.
    has_operation(&tokens) && !ends_incomplete(&tokens)
}

// --- Intent helpers ---

fn is_operand(t: &Token) -> bool {
    matches!(t, Token::Num(_) | Token::Const(_))
}

/// Does the expression actually do something to its operands? Implicit
/// multiplication counts: `2pi` is a calculation, `pi` on its own is a word.
/// A leading sign does not: `-5` is a number someone typed, not a subtraction.
fn has_operation(tokens: &[Token]) -> bool {
    let body = match tokens.first() {
        Some(Token::Op('-')) | Some(Token::Op('+')) => &tokens[1..],
        _ => tokens,
    };
    body.iter().any(|t| {
        matches!(
            t,
            Token::Op(_) | Token::Func(_) | Token::Factorial | Token::Percent | Token::LParen
        )
    }) || body
        .windows(2)
        .any(|w| is_operand(&w[0]) && matches!(w[1], Token::Const(_)))
}

/// True when the expression can't be complete: a trailing operator or an
/// unclosed paren.
fn ends_incomplete(tokens: &[Token]) -> bool {
    if matches!(tokens.last(), Some(Token::Op(_)) | Some(Token::Func(_))) {
        return true;
    }
    let mut depth = 0i32;
    for t in tokens {
        match t {
            Token::LParen => depth += 1,
            Token::RParen => depth -= 1,
            _ => {}
        }
    }
    depth != 0
}

/// Whether the character at `i` stands alone between whitespace.
fn is_spaced(chars: &[char], i: usize) -> bool {
    let before = i > 0 && chars[i - 1].is_whitespace();
    let after = chars.get(i + 1).is_some_and(|c| c.is_whitespace());
    before && after
}

/// Three or more all-digit groups joined by `-`, `/` or `.`.
fn is_grouped_numeric(s: &str) -> bool {
    let parts: Vec<&str> = s.split(['-', '/', '.']).collect();
    parts.len() >= 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

// --- Tokens ---

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    /// A named constant (`pi`, `e`). Distinct from `Num` so `2pi` can mean
    /// multiplication while `2 3` stays an error.
    Const(f64),
    Op(char),
    LParen,
    RParen,
    Func(String),
    Factorial,
    Percent,
}

// --- Tokenizer ---

fn tokenize(expr: &str, aliases: Aliases) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' => {
                let (value, next) = scan_number(&chars, i)?;
                tokens.push(Token::Num(value));
                i = next;
            }
            '+' | '-' | '^' => {
                tokens.push(Token::Op(c));
                i += 1;
            }
            '*' => {
                tokens.push(Token::Op('*'));
                i += 1;
            }
            // x/X as multiply alias. Free-standing only when the alias has to
            // earn it: `1920x1080` is a resolution, `3 x 4` is a product.
            'x' | 'X' if aliases == Aliases::Free => {
                tokens.push(Token::Op('*'));
                i += 1;
            }
            '/' => {
                tokens.push(Token::Op('/'));
                i += 1;
            }
            // : as divide alias, same deal - `16:9` and `10:30` are not
            // divisions, `10 : 2` is.
            ':' if aliases == Aliases::Free || is_spaced(&chars, i) => {
                tokens.push(Token::Op('/'));
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '!' => {
                tokens.push(Token::Factorial);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            'a'..='z' | 'A'..='Z' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let lower = word.to_lowercase();
                match lower.as_str() {
                    "pi" => tokens.push(Token::Const(consts::PI)),
                    "e" => tokens.push(Token::Const(consts::E)),
                    "sqrt" | "abs" | "round" | "floor" | "ceil" | "sin" | "cos" | "tan" | "log"
                    | "ln" => tokens.push(Token::Func(lower)),
                    // A lone x is the multiply alias; glued to digits it was
                    // never one, so `1920x1080` lands on the error below.
                    "x" => tokens.push(Token::Op('*')),
                    // v prefix as sqrt shorthand: v 16 -> sqrt(16)
                    "v" => tokens.push(Token::Func("sqrt".into())),
                    _ => return Err(format!("Unknown identifier: {word}")),
                }
            }
            _ => return Err(format!("Unknown character: {c}")),
        }
    }
    Ok(tokens)
}

/// Scan one number starting at `start`, accepting group separators (`1,500`)
/// and scientific notation (`1e6`, `2.5e-3`). Returns the value and the index
/// just past it.
fn scan_number(chars: &[char], start: usize) -> Result<(f64, usize), String> {
    let mut digits = String::new();
    let mut i = start;
    let mut seen_dot = false;

    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            digits.push(c);
            i += 1;
        } else if c == '.' {
            if seen_dot {
                return Err("Malformed number".into());
            }
            seen_dot = true;
            digits.push(c);
            i += 1;
        } else if c == ',' && is_group_separator(chars, i) {
            // Thousands separator: skip it so our own comma-formatted output
            // can be pasted straight back in.
            i += 1;
        } else {
            break;
        }
    }

    // Exponent, but only when digits actually follow - otherwise `2e` is the
    // constant e and means 2 * e.
    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        let mut j = i + 1;
        if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
            j += 1;
        }
        if j < chars.len() && chars[j].is_ascii_digit() {
            digits.push('e');
            if chars[i + 1] == '+' || chars[i + 1] == '-' {
                digits.push(chars[i + 1]);
            }
            while j < chars.len() && chars[j].is_ascii_digit() {
                digits.push(chars[j]);
                j += 1;
            }
            i = j;
        }
    }

    digits
        .parse::<f64>()
        .map(|v| (v, i))
        .map_err(|_| format!("Invalid number: {digits}"))
}

/// A comma is a thousands separator only when exactly three digits follow and
/// no fourth. `1,500` yes; `1,5` no (that is two numbers, and an error).
fn is_group_separator(chars: &[char], comma: usize) -> bool {
    let after = &chars[comma + 1..];
    after.len() >= 3
        && after[..3].iter().all(|c| c.is_ascii_digit())
        && !after.get(3).is_some_and(|c| c.is_ascii_digit())
}

// --- Parser (recursive descent) ---
//
// add_sub -> mul_div -> unary -> power -> postfix -> atom
//
// Unary minus sits *above* power so `-3^2` is `-(3^2)` = -9, matching Python,
// Google and every scientific calculator.

fn parse_add_sub(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_mul_div(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Op('+') => {
                *pos += 1;
                left += parse_mul_div(tokens, pos)?;
            }
            Token::Op('-') => {
                *pos += 1;
                left -= parse_mul_div(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_mul_div(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_unary(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Op('*') => {
                *pos += 1;
                left *= parse_unary(tokens, pos)?;
            }
            Token::Op('/') => {
                *pos += 1;
                let r = parse_unary(tokens, pos)?;
                if r == 0.0 {
                    return Err("Division by zero".into());
                }
                left /= r;
            }
            Token::Percent if is_modulo(tokens, *pos) => {
                *pos += 1;
                let r = parse_unary(tokens, pos)?;
                if r == 0.0 {
                    return Err("Modulo by zero".into());
                }
                left %= r;
            }
            // Implicit multiplication: `2(3+4)`, `2pi`, `3sqrt(9)`. Only before
            // a paren, function or constant - two bare numbers stay an error.
            Token::LParen | Token::Func(_) | Token::Const(_) => {
                left *= parse_unary(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

/// Disambiguate %: modulo if followed by something that starts an expression,
/// postfix percent otherwise.
fn is_modulo(tokens: &[Token], pos: usize) -> bool {
    if pos + 1 >= tokens.len() {
        return false; // end of input -> postfix percent
    }
    matches!(
        tokens[pos + 1],
        Token::Num(_) | Token::Const(_) | Token::LParen | Token::Func(_)
    )
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    match tokens.get(*pos) {
        Some(Token::Op('-')) => {
            *pos += 1;
            Ok(-parse_unary(tokens, pos)?)
        }
        Some(Token::Op('+')) => {
            *pos += 1;
            parse_unary(tokens, pos)
        }
        _ => parse_power(tokens, pos),
    }
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let base = parse_postfix(tokens, pos)?;
    if matches!(tokens.get(*pos), Some(Token::Op('^'))) {
        *pos += 1;
        // Right-associative, and `2^-3` reads the sign as part of the exponent.
        let exp = parse_unary(tokens, pos)?;
        Ok(base.powf(exp))
    } else {
        Ok(base)
    }
}

fn parse_postfix(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut val = parse_atom(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Factorial => {
                *pos += 1;
                val = factorial(val)?;
            }
            Token::Percent if !is_modulo(tokens, *pos) => {
                *pos += 1;
                val /= 100.0;
            }
            _ => break,
        }
    }
    Ok(val)
}

fn parse_atom(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let Some(token) = tokens.get(*pos) else {
        return Err("Unexpected end of expression".into());
    };
    match token {
        Token::Num(n) | Token::Const(n) => {
            let v = *n;
            *pos += 1;
            Ok(v)
        }
        Token::Func(name) => {
            let name = name.clone();
            *pos += 1;
            if !matches!(tokens.get(*pos), Some(Token::LParen)) {
                // Shorthand without parens: `sqrt 16`, `v 16`.
                let arg = parse_unary(tokens, pos)?;
                return apply_func(&name, arg);
            }
            *pos += 1; // skip (
            let arg = parse_add_sub(tokens, pos)?;
            if !matches!(tokens.get(*pos), Some(Token::RParen)) {
                return Err("Missing closing parenthesis".into());
            }
            *pos += 1; // skip )
            apply_func(&name, arg)
        }
        Token::LParen => {
            *pos += 1;
            let v = parse_add_sub(tokens, pos)?;
            if !matches!(tokens.get(*pos), Some(Token::RParen)) {
                return Err("Missing closing parenthesis".into());
            }
            *pos += 1;
            Ok(v)
        }
        _ => Err(format!("Unexpected token: {token:?}")),
    }
}

fn apply_func(name: &str, arg: f64) -> Result<f64, String> {
    match name {
        "sqrt" => {
            if arg < 0.0 {
                Err("Square root of negative number".into())
            } else {
                Ok(arg.sqrt())
            }
        }
        "abs" => Ok(arg.abs()),
        "round" => Ok(arg.round()),
        "floor" => Ok(arg.floor()),
        "ceil" => Ok(arg.ceil()),
        "sin" => Ok(arg.sin()),
        "cos" => Ok(arg.cos()),
        "tan" => Ok(arg.tan()),
        "log" => {
            if arg <= 0.0 {
                Err("Logarithm of non-positive number".into())
            } else {
                Ok(arg.log10())
            }
        }
        "ln" => {
            if arg <= 0.0 {
                Err("Logarithm of non-positive number".into())
            } else {
                Ok(arg.ln())
            }
        }
        _ => Err(format!("Unknown function: {name}")),
    }
}

fn factorial(v: f64) -> Result<f64, String> {
    if v < 0.0 || v != v.trunc() {
        return Err("Factorial requires a non-negative integer".into());
    }
    let n = v as u64;
    if n > MAX_FACTORIAL {
        return Err(format!("Factorial too large (max {MAX_FACTORIAL}!)"));
    }
    let mut result = 1.0_f64;
    for i in 2..=n {
        result *= i as f64;
    }
    Ok(result)
}

// --- Formatting ---

/// Grouped for display: `1,000,000`.
pub fn format_display(v: f64) -> String {
    format_number(v, true)
}

/// Bare and re-parseable, for the clipboard: `1000000`.
pub fn format_raw(v: f64) -> String {
    format_number(v, false)
}

fn format_number(v: f64, group: bool) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let magnitude = v.abs();
    if !(SCI_LOWER..SCI_UPPER).contains(&magnitude) {
        return format_scientific(v);
    }
    if v == v.trunc() {
        let int = format!("{}", v as i64);
        return if group { group_digits(&int) } else { int };
    }
    let s = format!("{v:.DECIMALS$}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    match s.split_once('.') {
        Some((int, dec)) if group => format!("{}.{}", group_digits(int), dec),
        _ => s.to_string(),
    }
}

fn format_scientific(v: f64) -> String {
    let s = format!("{v:.DECIMALS$e}");
    // Trim the mantissa's trailing zeros: 1.2000000000e30 -> 1.2e30
    match s.split_once('e') {
        Some((mantissa, exp)) => {
            let m = mantissa.trim_end_matches('0').trim_end_matches('.');
            format!("{m}e{exp}")
        }
        None => s,
    }
}

/// Insert thousands separators into an already-formatted integer string.
fn group_digits(int: &str) -> String {
    let (sign, digits) = match int.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", int),
    };
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(expr: &str) -> String {
        eval(expr).unwrap().display
    }

    fn raw(expr: &str) -> String {
        eval(expr).unwrap().raw
    }

    #[test]
    fn basic_arithmetic() {
        assert_eq!(calc("2 + 3"), "5");
        assert_eq!(calc("10 - 4"), "6");
        assert_eq!(calc("3 * 7"), "21");
        assert_eq!(calc("15 / 3"), "5");
    }

    #[test]
    fn order_of_operations() {
        assert_eq!(calc("2 + 3 * 4"), "14");
        assert_eq!(calc("(2 + 3) * 4"), "20");
    }

    #[test]
    fn power() {
        assert_eq!(calc("2 ^ 10"), "1,024");
        assert_eq!(calc("2 ^ -2"), "0.25");
    }

    /// Unary minus binds looser than `^`, so this is -(3^2).
    #[test]
    fn negation_binds_looser_than_power() {
        assert_eq!(calc("-3^2"), "-9");
        assert_eq!(calc("(-3)^2"), "9");
    }

    #[test]
    fn modulo() {
        assert_eq!(calc("100 % 7"), "2");
    }

    #[test]
    fn constants() {
        assert!(calc("pi").starts_with("3.14"));
        assert!(calc("e").starts_with("2.71"));
    }

    #[test]
    fn functions() {
        assert_eq!(calc("sqrt(16)"), "4");
        assert_eq!(calc("abs(-5)"), "5");
        assert_eq!(calc("round(3.7)"), "4");
        assert_eq!(calc("floor(3.9)"), "3");
        assert_eq!(calc("ceil(3.1)"), "4");
    }

    #[test]
    fn factorial_cases() {
        assert_eq!(calc("5!"), "120");
        assert_eq!(calc("0!"), "1");
        assert_eq!(calc("10!"), "3,628,800");
    }

    #[test]
    fn percent_postfix() {
        assert_eq!(calc("50%"), "0.5");
        assert_eq!(calc("200 * 15%"), "30");
    }

    #[test]
    fn multiply_aliases() {
        assert_eq!(calc("3 x 4"), "12");
        assert_eq!(calc("10 : 2"), "5");
    }

    #[test]
    fn sqrt_shorthand() {
        assert_eq!(calc("v 16"), "4");
        assert_eq!(calc("sqrt 16"), "4");
    }

    #[test]
    fn implicit_multiplication() {
        assert_eq!(calc("2(3 + 4)"), "14");
        assert_eq!(calc("3sqrt(9)"), "9");
        assert!(calc("2pi").starts_with("6.28"));
        // Two bare numbers is still a typo, not a product.
        assert!(eval("2 3").is_err());
    }

    // --- Regressions: each of these was wrong before core/calc ---

    /// `1/100000` used to print as "0": a wrong answer, not an error.
    #[test]
    fn small_magnitudes_keep_their_digits() {
        assert_eq!(calc("1 / 100000"), "0.00001");
        assert_eq!(calc("2 / 300000"), "0.0000066667");
    }

    /// The formatter emits `1,500`, so the tokenizer has to accept it back.
    #[test]
    fn comma_grouped_input_round_trips() {
        assert_eq!(calc("1,500 + 1"), "1,501");
        assert_eq!(calc("1,000,000 / 2"), "500,000");
        // Not a group separator: two numbers, and an error.
        assert!(eval("1,5").is_err());
    }

    #[test]
    fn scientific_input() {
        assert_eq!(calc("1e6"), "1,000,000");
        assert_eq!(calc("2.5e-3 * 1000"), "2.5");
    }

    /// A hard 1e12 ceiling used to reject ordinary results.
    #[test]
    fn large_results_are_not_rejected() {
        assert_eq!(calc("9999999999999 * 100"), "999,999,999,999,900");
        assert_eq!(calc("2 ^ 100"), "1.2676506002e30");
    }

    #[test]
    fn float_noise_is_hidden() {
        assert_eq!(calc("0.1 + 0.2"), "0.3");
        assert_eq!(calc("1.1 * 3"), "3.3");
    }

    #[test]
    fn raw_output_is_paste_safe() {
        assert_eq!(raw("1000 * 1000"), "1000000");
        assert_eq!(calc("1000 * 1000"), "1,000,000");
        assert_eq!(raw("-2500 - 500"), "-3000");
    }

    #[test]
    fn errors() {
        assert!(eval("1 / 0").is_err());
        assert!(eval("(-1)!").is_err());
        assert!(eval("sqrt(-4)").is_err());
        assert!(eval("2 +").is_err());
    }

    // --- Intent gate ---

    /// The cases that must never be evaluated: things people name files after.
    #[test]
    fn file_and_date_shapes_are_not_math() {
        for q in [
            "20-05-2026", // the folder name from issue #325
            "2026-08-04",
            "10-5-26",
            "1.2.3",       // version
            "192.168.1.1", // address
            "~/Documents/2",
            "/usr/2",
            "part-1",
            "look 2",
            "chapter-2",
            "hello",
            "1920x1080", // resolution: the alias is glued to the digits
            "16:9",      // ratio
            "10:30",     // time
            "42",        // a bare number is not a calculation
            "2 +",       // half-typed
            "(2 + 3",    // unbalanced
            "",
        ] {
            assert!(!is_math(q), "{q} should not be math");
        }
    }

    /// The whole point: how you space an expression is a typing habit, not an
    /// intent. Both halves of every pair behave identically.
    #[test]
    fn spacing_never_changes_the_answer() {
        for (tight, spaced) in [
            ("1/1000", "1 / 1000"),
            ("2+2", "2 + 2"),
            ("2-3", "2 - 3"),
            ("100*7", "100 * 7"),
            ("2^10", "2 ^ 10"),
        ] {
            assert!(is_math(tight), "{tight} should be math");
            assert!(is_math(spaced), "{spaced} should be math");
            assert_eq!(calc(tight), calc(spaced));
        }
    }

    #[test]
    fn real_math_is_recognised() {
        for q in [
            "2+2",
            "1/1000",
            "2 - 3",
            "1920 x 1080",
            "sqrt(16)",
            "50%",
            "5!",
            "2^10",
            "(2+3)*4",
            "1,500 + 1",
            "100 / 7",
            "2pi",
            "24/7",
            "10 : 2",
        ] {
            assert!(is_math(q), "{q} should be math");
        }
    }

    /// The dedicated /calc panel is past the gate - the user already said this
    /// is a calculation, so the aliases are honoured wherever they land.
    #[test]
    fn calc_panel_accepts_glued_aliases() {
        assert_eq!(calc("1920x1080"), "2,073,600");
        assert_eq!(calc("16:9"), "1.7777777778");
    }

    // --- The inline path ---

    #[test]
    fn eval_query_answers_or_stays_quiet() {
        assert_eq!(eval_query("1/1000").unwrap().display, "0.001");
        assert_eq!(eval_query("2+2").unwrap().raw, "4");
        assert!(eval_query("20-05-2026").is_none());
        assert!(eval_query("look 2").is_none());
        assert!(eval_query("2 +").is_none());
    }

    /// The `=?` habit the old AI-card trigger accidentally trained. It costs
    /// nothing to keep honouring it.
    #[test]
    fn trailing_equals_and_question_marks_are_stripped() {
        for q in ["2+2=", "2+2?", "2+2=?", "2+2 = ?", "1 / 1000 =?"] {
            assert!(eval_query(q).is_some(), "{q} should still evaluate");
        }
        assert_eq!(eval_query("2+2=?").unwrap().display, "4");
    }
}
