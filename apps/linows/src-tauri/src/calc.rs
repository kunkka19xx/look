//! Tauri surface for the calculator. The evaluator itself lives in
//! `core/calc` (`look-calc`) so linows and macOS can't drift apart.

/// Evaluate an expression the user has already declared to be one (the /calc
/// panel). Returns it formatted for display.
#[tauri::command]
pub fn eval_calc(expr: String) -> Result<String, String> {
    look_calc::eval(&expr).map(|c| c.display)
}

/// The main search field's calculator: answers only when the query was clearly
/// meant as arithmetic, `None` otherwise. Local and allocation-light, so the
/// frontend calls it on every keystroke without a debounce.
#[tauri::command]
pub fn calc_inline(query: String) -> Option<look_calc::Calculation> {
    look_calc::eval_query(&query)
}
