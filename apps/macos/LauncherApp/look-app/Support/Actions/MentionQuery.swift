import Foundation

/// The `@token` being typed in the AI input, used to drive the file-mention
/// popup. Pure text math so the interaction is unit-tested rather than
/// discovered by clicking around.
///
/// Deliberately narrow, because `@` already means something here: `explicit.rs`
/// splits `>add lunch @ 1pm` on it. A mention needs the `@` to open a word
/// (start of line or after whitespace) and to be followed IMMEDIATELY by
/// non-space text, so the date form never triggers one. Nothing is attached
/// until the user picks from the popup, so typing `@1pm` and carrying on
/// leaves the text exactly as written.
nonisolated enum MentionQuery {
    struct Active: Equatable {
        /// The text after `@`, what the file search runs on.
        let token: String
        /// Offsets of `@` and of the caret, in Characters from the start.
        let start: Int
        let end: Int
    }

    /// The mention being typed at `caret` (a Character offset), or nil.
    static func active(in text: String, caret: Int) -> Active? {
        let chars = Array(text)
        let caret = max(0, min(caret, chars.count))
        guard caret > 0 else { return nil }

        var index = caret - 1
        while index >= 0 {
            let c = chars[index]
            if c == "@" { break }
            // A mention is one word: whitespace ends the search, and so does a
            // second `@` boundary.
            if c.isWhitespace || c.isNewline { return nil }
            index -= 1
        }
        guard index >= 0, chars[index] == "@" else { return nil }

        // "foo@bar" is an email, not a mention: the `@` must open a word.
        if index > 0 {
            let before = chars[index - 1]
            guard before.isWhitespace || before.isNewline else { return nil }
        }

        let token = String(chars[(index + 1)..<caret])
        // An empty token would search for everything the moment `@` is typed.
        guard !token.isEmpty else { return nil }
        return Active(token: token, start: index, end: caret)
    }

    /// `text` with the mention removed, plus where the caret lands. Accepting a
    /// file consumes the token so the submitted query is clean prose and
    /// `explicit.rs` never sees a stray `@`.
    static func consume(_ text: String, _ active: Active) -> (text: String, caret: Int) {
        let chars = Array(text)
        let head = String(chars[0..<active.start])
        let tail = String(chars[min(active.end, chars.count)...])
        return (head + tail, active.start)
    }
}
