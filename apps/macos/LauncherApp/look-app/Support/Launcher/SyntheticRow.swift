import Foundation

/// What kind of synthesized (not-a-real-candidate) result row this is,
/// classified once from its id prefix instead of re-derived independently by
/// every caller. The per-kind `fromResultID` accessors in `AppConstants.swift`
/// stay as the underlying primitives - `classify(resultID:)` just calls each
/// of them once, in one place.
enum SyntheticRow {
    case prefixSuggestion(prefix: String)
    case webSuggestion(text: String)
    case commandSuggestion(commandID: String)
    case webURL(url: String)
    case calc(raw: String)

    static func classify(resultID: String) -> SyntheticRow? {
        if let prefix = AppConstants.Launcher.PrefixSuggestion.prefix(fromResultID: resultID) {
            return .prefixSuggestion(prefix: prefix)
        }
        if let text = AppConstants.Launcher.WebSuggestion.text(fromResultID: resultID) {
            return .webSuggestion(text: text)
        }
        if let commandID = AppConstants.Launcher.CommandSuggestion.commandID(fromResultID: resultID) {
            return .commandSuggestion(commandID: commandID)
        }
        if let url = AppConstants.Launcher.WebURL.url(fromResultID: resultID) {
            return .webURL(url: url)
        }
        if let raw = AppConstants.Launcher.Calc.rawValue(fromResultID: resultID) {
            return .calc(raw: raw)
        }
        return nil
    }
}
