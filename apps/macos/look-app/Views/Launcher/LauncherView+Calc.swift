import Foundation

/// The pinned calculator row: evaluation and intent-detection ("is this
/// arithmetic, not a date/resolution/ratio?") live in the shared `core/calc`
/// crate via `EngineBridge`, so this file is presentation and placement only.
/// Mirrors `LauncherView+URLResults.swift`.
extension LauncherView {
    /// A synthesized row for the current query's arithmetic answer, or nil.
    /// Network-free; cheap enough to call on every keystroke.
    var calcResult: LauncherResult? {
        guard allowsSuggestionRows, let calc = bridge.calcInline(query: query) else { return nil }
        let expr = query.trimmingCharacters(in: .whitespacesAndNewlines)

        var result = LauncherResult(
            id: "\(AppConstants.Launcher.Calc.resultIDPrefix)\(calc.raw)",
            kind: .app,
            title: calc.display,
            subtitle: "\(expr)  •  \(AppConstants.Launcher.Calc.enterToCopyHint)",
            path: calc.raw,
            score: .max
        )
        result.calcExpression = expr
        result.calcRawValue = calc.raw
        return result
    }
}
