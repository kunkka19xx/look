//! The launcher's surface for the calculator. The evaluator itself lives in
//! `core/calc` (`look-calc`) so linows and macOS can't drift apart.

/// For the /calc panel, where the user already declared this is arithmetic.
pub fn eval_calc(expr: &str) -> Result<String, String> {
    look_calc::eval(expr).map(|c| c.display)
}

/// The main search field: answers only when the query was meant as arithmetic.
/// Cheap enough to call on every keystroke.
pub fn calc_inline(query: &str) -> Option<look_calc::Calculation> {
    look_calc::eval_query(query)
}
