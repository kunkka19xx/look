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

    /// Joins with exactly one trailing newline, preserving the file's shape. A write
    /// changes only the keys the caller upserted or removed: blank lines, comments,
    /// and their order all survive, because an update must never reformat a config the
    /// user is entitled to lay out however they like.
    static func render(_ lines: [String]) -> String {
        lines.joined(separator: "\n") + "\n"
    }

    /// The section header that builds before this type emitted as garbage. They tested
    /// for it by comparing against its comment-stripped form (always the empty string,
    /// so never a match) and appended a fresh copy plus a blank line on every save.
    /// Named here only so the damage can be recognised and undone. Nothing in normal
    /// operation reads it, and no writer may depend on it being present.
    private static let legacySectionHeader = "# UI theme"

    /// Undoes the damage described on `legacySectionHeader`, returning nil when there is
    /// nothing to undo.
    ///
    /// Deliberately narrow. It repairs a file only when it carries the signature of the
    /// bug (the legacy header more than once), and even then it removes only the surplus
    /// copies of that one header plus the blank runs they came with. A hand-written
    /// config, including one that repeats `####` dividers or spaces sections with double
    /// blanks, matches no signature and is returned untouched.
    static func repairingLegacyDamage(_ raw: String) -> String? {
        let lines = parse(raw)
        guard lines.filter({ trim($0) == legacySectionHeader }).count > 1 else {
            return nil
        }

        var kept: [String] = []
        var keptHeader = false

        for line in lines {
            let trimmed = trim(line)

            if trimmed == legacySectionHeader {
                guard !keptHeader else {
                    continue
                }
                keptHeader = true
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

        let repaired = render(kept)
        return repaired == raw ? nil : repaired
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

    /// Drops a trailing comment. `#` only starts one at the beginning of the line or
    /// after whitespace, so it survives inside a value: cutting at the first `#`
    /// anywhere would truncate a path like `/Users/me/pic#1.png` down to `/Users/me/pic`
    /// on read, silently corrupting the setting.
    static func stripComment(_ line: String) -> String {
        var index = line.startIndex
        while index < line.endIndex {
            if line[index] == Character(commentPrefix),
               index == line.startIndex || line[line.index(before: index)].isWhitespace
            {
                return String(line[..<index])
            }
            index = line.index(after: index)
        }
        return line
    }

    private static func isComment(_ trimmedLine: String) -> Bool {
        trimmedLine.hasPrefix(commentPrefix)
    }

    private static func trim(_ line: String) -> String {
        line.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
