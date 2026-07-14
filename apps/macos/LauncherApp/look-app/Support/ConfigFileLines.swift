import Foundation

/// The single line-level editor for `~/.look.config`. Every writer goes through
/// `parse` -> `upsert`/`remove` -> `render` so the file's shape is decided in one
/// place instead of once per feature.
///
/// Two bugs came from not having this. `render` exists because both writers used
/// `lines.joined(separator: "\n") + "\n"` on a parse that keeps the trailing empty
/// element, so every save appended one more blank line. And `normalize` exists
/// because a writer treated a `# UI theme` comment as an anchor, testing for it by
/// comparing against its comment-stripped form (always the empty string, so never a
/// match) and appending a fresh copy on every save.
///
/// The rule those bugs teach: comments belong to the user, who may rename, reorder,
/// or delete any of them, so no writer may depend on one. Nothing here reads a
/// comment's text, only its `#` prefix.
enum ConfigFileLines {
    private static let commentPrefix = "#"
    private static let keyValueSeparator: Character = "="

    /// Splits into logical lines. The file's terminating newline is a terminator, not
    /// a line, so the empty element it produces is dropped: leaving it in means
    /// `upsert` appends new keys *behind* a blank, which is how stray gaps ended up
    /// in front of keys that were added after the file was first written.
    static func parse(_ raw: String) -> [String] {
        var lines = raw.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).map(String.init)
        while let last = lines.last, trim(last).isEmpty {
            lines.removeLast()
        }
        return lines
    }

    /// Normalizes, then joins with exactly one trailing newline. Feeding the result
    /// back through `parse` and `render` yields the same text, so repeated saves
    /// cannot grow the file.
    static func render(_ lines: [String]) -> String {
        normalize(lines).joined(separator: "\n") + "\n"
    }

    /// Collapses blank-line runs, trims leading and trailing blanks, and keeps only
    /// the first occurrence of each distinct comment. A repeated comment is the same
    /// text as one already present, so dropping it removes nothing the reader did not
    /// already have. Every key line survives.
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

    /// Rewrites `key` in place, or appends it when absent.
    static func upsert(_ lines: inout [String], key: String, value: String) {
        let assignment = "\(key)\(keyValueSeparator)"
        for index in lines.indices where trim(stripComment(lines[index])).hasPrefix(assignment) {
            lines[index] = "\(key)\(keyValueSeparator)\(value)"
            return
        }
        lines.append("\(key)\(keyValueSeparator)\(value)")
    }

    static func remove(_ lines: inout [String], key: String) {
        let assignment = "\(key)\(keyValueSeparator)"
        lines.removeAll { trim(stripComment($0)).hasPrefix(assignment) }
    }

    static func keyValues(_ raw: String) -> [String: String] {
        var values: [String: String] = [:]
        for line in parse(raw) {
            let stripped = trim(stripComment(line))
            guard let separator = stripped.firstIndex(of: keyValueSeparator) else {
                continue
            }
            let key = trim(String(stripped[..<separator]))
            guard !key.isEmpty else {
                continue
            }
            values[key] = trim(String(stripped[stripped.index(after: separator)...]))
        }
        return values
    }

    static func stripComment(_ line: String) -> String {
        guard let start = line.firstIndex(of: Character(commentPrefix)) else {
            return line
        }
        return String(line[..<start])
    }

    private static func isComment(_ trimmedLine: String) -> Bool {
        trimmedLine.hasPrefix(commentPrefix)
    }

    private static func trim(_ line: String) -> String {
        line.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
