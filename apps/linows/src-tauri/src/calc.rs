use std::f64::consts;

const MAX_RESULT: f64 = 1_000_000_000_000.0;
const MAX_FACTORIAL: u64 = 170;

#[tauri::command]
pub fn eval_calc(expr: String) -> Result<String, String> {
    eval_expression(&expr).map(|v| format_number(v))
}

fn eval_expression(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    if tokens.is_empty() {
        return Err("Empty expression".into());
    }
    let mut pos = 0;
    let result = parse_add_sub(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(format!("Unexpected token: {:?}", tokens[pos]));
    }
    if result.is_nan() || result.is_infinite() {
        return Err("Result is undefined".into());
    }
    if result.abs() > MAX_RESULT {
        return Err("Result too large".into());
    }
    Ok(result)
}

// --- Tokens ---

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Op(char),
    LParen,
    RParen,
    Func(String),
    Factorial,
    Percent,
}

// --- Tokenizer ---

fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | ',' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        num.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Num(
                    num.parse().map_err(|_| format!("Invalid number: {num}"))?,
                ));
            }
            '+' | '-' | '^' => {
                tokens.push(Token::Op(c));
                chars.next();
            }
            '*' | 'x' | 'X' => {
                // x/X as multiply alias
                tokens.push(Token::Op('*'));
                chars.next();
            }
            '/' | ':' => {
                // : as divide alias
                tokens.push(Token::Op('/'));
                chars.next();
            }
            '%' => {
                tokens.push(Token::Percent);
                chars.next();
            }
            '!' => {
                tokens.push(Token::Factorial);
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            'a'..='z' | 'A'..='Z' => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        word.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let lower = word.to_lowercase();
                match lower.as_str() {
                    "pi" => tokens.push(Token::Num(consts::PI)),
                    "e" => tokens.push(Token::Num(consts::E)),
                    "sqrt" | "abs" | "round" | "floor" | "ceil"
                    | "sin" | "cos" | "tan" | "log" | "ln" => {
                        tokens.push(Token::Func(lower));
                    }
                    // v/V prefix as sqrt shorthand: v16 → sqrt(16)
                    _ if lower.starts_with('v') && word.len() == 1 => {
                        tokens.push(Token::Func("sqrt".into()));
                    }
                    _ => return Err(format!("Unknown identifier: {word}")),
                }
            }
            _ => return Err(format!("Unknown character: {c}")),
        }
    }
    Ok(tokens)
}

// --- Parser (recursive descent) ---

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
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Op('*') => {
                *pos += 1;
                left *= parse_power(tokens, pos)?;
            }
            Token::Op('/') => {
                *pos += 1;
                let r = parse_power(tokens, pos)?;
                if r == 0.0 {
                    return Err("Division by zero".into());
                }
                left /= r;
            }
            Token::Percent if is_modulo(tokens, *pos) => {
                *pos += 1;
                let r = parse_power(tokens, pos)?;
                if r == 0.0 {
                    return Err("Modulo by zero".into());
                }
                left %= r;
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
        return false; // end of input → postfix percent
    }
    matches!(
        tokens[pos + 1],
        Token::Num(_) | Token::LParen | Token::Func(_)
    )
}

fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let base = parse_unary(tokens, pos)?;
    if *pos < tokens.len() && matches!(tokens[*pos], Token::Op('^')) {
        *pos += 1;
        let exp = parse_power(tokens, pos)?; // right-associative
        Ok(base.powf(exp))
    } else {
        Ok(base)
    }
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos < tokens.len() && matches!(tokens[*pos], Token::Op('-')) {
        *pos += 1;
        Ok(-parse_postfix(tokens, pos)?)
    } else if *pos < tokens.len() && matches!(tokens[*pos], Token::Op('+')) {
        *pos += 1;
        parse_postfix(tokens, pos)
    } else {
        parse_postfix(tokens, pos)
    }
}

fn parse_postfix(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut val = parse_atom(tokens, pos)?;
    // Handle postfix ! and % (but not % when it's modulo)
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
    if *pos >= tokens.len() {
        return Err("Unexpected end of expression".into());
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let v = *n;
            *pos += 1;
            Ok(v)
        }
        Token::Func(name) => {
            let name = name.clone();
            *pos += 1;
            // Expect ( arg )
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::LParen) {
                // Allow sqrt shorthand without parens: sqrt 16
                let arg = parse_unary(tokens, pos)?;
                return apply_func(&name, arg);
            }
            *pos += 1; // skip (
            let arg = parse_add_sub(tokens, pos)?;
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RParen) {
                return Err("Missing closing parenthesis".into());
            }
            *pos += 1; // skip )
            apply_func(&name, arg)
        }
        Token::LParen => {
            *pos += 1;
            let v = parse_add_sub(tokens, pos)?;
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RParen) {
                return Err("Missing closing parenthesis".into());
            }
            *pos += 1;
            Ok(v)
        }
        _ => Err(format!("Unexpected token: {:?}", tokens[*pos])),
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

fn format_number(v: f64) -> String {
    // Integer check
    if v == v.trunc() && v.abs() < 1e15 {
        let n = v as i64;
        return format_with_commas_int(n);
    }
    // Up to 4 decimal places, strip trailing zeros
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    // Add commas to integer part
    if let Some(dot_pos) = s.find('.') {
        let int_part = &s[..dot_pos];
        let dec_part = &s[dot_pos..];
        if let Ok(n) = int_part.parse::<i64>() {
            format!("{}{}", format_with_commas_int(n), dec_part)
        } else {
            s.to_string()
        }
    } else if let Ok(n) = s.parse::<i64>() {
        format_with_commas_int(n)
    } else {
        s.to_string()
    }
}

fn format_with_commas_int(n: i64) -> String {
    let neg = n < 0;
    let s = n.unsigned_abs().to_string();
    let bytes = s.as_bytes();
    let mut result = Vec::new();
    for (i, &b) in bytes.iter().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(b',');
        }
        result.push(b);
    }
    result.reverse();
    let s = String::from_utf8(result).unwrap();
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expr: &str) -> Result<String, String> {
        eval_expression(expr).map(|v| format_number(v))
    }

    #[test]
    fn basic_arithmetic() {
        assert_eq!(eval("2 + 3").unwrap(), "5");
        assert_eq!(eval("10 - 4").unwrap(), "6");
        assert_eq!(eval("3 * 7").unwrap(), "21");
        assert_eq!(eval("15 / 3").unwrap(), "5");
    }

    #[test]
    fn order_of_operations() {
        assert_eq!(eval("2 + 3 * 4").unwrap(), "14");
        assert_eq!(eval("(2 + 3) * 4").unwrap(), "20");
    }

    #[test]
    fn power() {
        assert_eq!(eval("2 ^ 10").unwrap(), "1,024");
    }

    #[test]
    fn modulo() {
        assert_eq!(eval("100 % 7").unwrap(), "2");
    }

    #[test]
    fn constants() {
        assert!(eval("pi").unwrap().starts_with("3.14"));
        assert!(eval("e").unwrap().starts_with("2.71"));
    }

    #[test]
    fn functions() {
        assert_eq!(eval("sqrt(16)").unwrap(), "4");
        assert_eq!(eval("abs(-5)").unwrap(), "5");
        assert_eq!(eval("round(3.7)").unwrap(), "4");
        assert_eq!(eval("floor(3.9)").unwrap(), "3");
        assert_eq!(eval("ceil(3.1)").unwrap(), "4");
    }

    #[test]
    fn factorial() {
        assert_eq!(eval("5!").unwrap(), "120");
        assert_eq!(eval("0!").unwrap(), "1");
        assert_eq!(eval("10!").unwrap(), "3,628,800");
    }

    #[test]
    fn percent_postfix() {
        assert_eq!(eval("50%").unwrap(), "0.5");
        assert_eq!(eval("200 * 15%").unwrap(), "30");
    }

    #[test]
    fn multiply_aliases() {
        assert_eq!(eval("3 x 4").unwrap(), "12");
        assert_eq!(eval("10 : 2").unwrap(), "5");
    }

    #[test]
    fn sqrt_shorthand() {
        assert_eq!(eval("v 16").unwrap(), "4");
    }

    #[test]
    fn comma_formatting() {
        assert_eq!(eval("1000 * 1000").unwrap(), "1,000,000");
    }

    #[test]
    fn decimal_precision() {
        assert_eq!(eval("1 / 3").unwrap(), "0.3333");
        assert_eq!(eval("3.14 * 2").unwrap(), "6.28");
    }

    #[test]
    fn errors() {
        assert!(eval("1 / 0").is_err());
        assert!(eval("(-1)!").is_err());
        assert!(eval("sqrt(-4)").is_err());
    }
}
