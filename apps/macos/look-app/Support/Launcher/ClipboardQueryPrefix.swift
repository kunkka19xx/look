import Foundation

/// Which clipboard history a query is asking for. One rule for both: the two
/// spellings are a character apart, and callers that trim and drop each by hand
/// eventually disagree about which history `ci"logo` meant.
enum ClipboardQueryPrefix {
    case text
    case image

    var prefix: String {
        switch self {
        case .text:
            return AppConstants.Launcher.QueryPrefix.clipboard
        case .image:
            return AppConstants.Launcher.QueryPrefix.clipboardImage
        }
    }

    func matches(_ query: String) -> Bool {
        searchTerm(in: query) != nil
    }

    /// What follows the prefix, or nil when the query is not this history's.
    func searchTerm(in query: String) -> String? {
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard normalized.lowercased().hasPrefix(prefix) else { return nil }
        return String(normalized.dropFirst(prefix.count))
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
