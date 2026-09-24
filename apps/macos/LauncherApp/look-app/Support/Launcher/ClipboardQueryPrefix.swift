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
        if self == .text && ClipboardQueryPrefix.image.matches(query) {
            return nil
        }
        let prefixName = self == .image ? "ci" : "c"
        return AppConstants.Launcher.QueryPrefix.strip(from: query, prefixName: prefixName)
    }
}
