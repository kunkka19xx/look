//! Tauri surface for the calculator. The evaluator itself lives in
//! `core/calc` (`look-calc`) so linows and macOS can't drift apart.

/// Evaluate an expression and return it formatted for display.
#[tauri::command]
pub fn eval_calc(expr: String) -> Result<String, String> {
    look_calc::eval(&expr).map(|c| c.display)
}
