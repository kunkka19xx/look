import Foundation

/// What kind of synthesized (not-a-real-candidate) result row this is,
/// classified from its id prefix. The prefixes are disjoint, so at most one
/// arm can match. Mirrors `classifyResultId` in the linows `catalog.js`.
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
