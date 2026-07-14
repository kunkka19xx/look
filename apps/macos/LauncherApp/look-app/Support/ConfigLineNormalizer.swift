import Foundation

/// Structural cleanup for `~/.look.config` applied on every write.
///
/// Comments in the config belong to the user: they can rename, reorder, or delete
/// any of them, so no writer may treat a comment as an anchor. An earlier build did,
/// testing for a `# UI theme` header by comparing it to its comment-stripped form
/// (always the empty string, so never a match) and appending a fresh header plus a
/// blank line on every save. Configs in the wild carry one such pair per save.
///
/// The rules below are therefore about shape, not text. They know nothing about any
/// particular comment, which is what makes them safe to run on a file the user edits
/// by hand.
enum ConfigLineNormalizer {
    private static let commentPrefix = "#"

    /// Collapses blank-line runs, trims leading and trailing blanks, and keeps only
    /// the first occurrence of each distinct comment. A repeated comment is the same
    /// text as one already in the file, so dropping it removes no information the
    /// reader did not already have. Every key line survives, and the result is stable
    /// under repeated application.
    static func normalize(_ lines: [String]) -> [String] {
        var kept: [String] = []
        var seenComments: Set<String> = []

        for line in lines {
            let trimmed = trim(line)

            if isComment(trimmed) {
                guard seenComments.insert(trimmed).inserted else {
                    continue
                }
                kept.append(line)
                continue
            }

            if trimmed.isEmpty, kept.last.map({ trim($0).isEmpty }) ?? true {
                continue
            }

            kept.append(line)
        }

        while let last = kept.last, trim(last).isEmpty {
            kept.removeLast()
        }

        return kept
    }

    private static func isComment(_ trimmedLine: String) -> Bool {
        trimmedLine.hasPrefix(commentPrefix)
    }

    private static func trim(_ line: String) -> String {
        line.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
