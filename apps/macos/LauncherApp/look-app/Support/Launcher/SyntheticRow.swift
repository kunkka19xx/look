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
    /// The planner-proposed action row in the main bar (Enter performs it).
    case aiAction(toolID: String)
    /// "Join <meeting>" for a `join` query (Enter opens the conferencing link).
    case meeting(url: String)
    /// "Call <name>" for a `call` query (Enter opens FaceTime or Messages).
    case call(url: String)

    static func classify(resultID: String) -> SyntheticRow? {
        if let toolID = AppConstants.Launcher.AIAction.toolID(fromResultID: resultID) {
            return .aiAction(toolID: toolID)
        }
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
        if let url = AppConstants.Launcher.Meeting.url(fromResultID: resultID) {
            return .meeting(url: url)
        }
        if let url = AppConstants.Launcher.Call.url(fromResultID: resultID) {
            return .call(url: url)
        }
        return nil
    }
}
