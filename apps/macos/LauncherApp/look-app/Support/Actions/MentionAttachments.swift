import Foundation

/// Files the user attached to the next AI turn with `@`. Ordered, deduped, and
/// budgeted: the model's context is finite and Ollama truncates a too-long
/// prompt SILENTLY, so the bar reports what it is carrying rather than letting
/// the user find out from a half-informed answer.
nonisolated struct MentionAttachments: Equatable {

    private(set) var files: [Attached] = []

    struct Attached: Equatable, Identifiable {
        let path: String
        /// The text as read WHEN IT WAS ATTACHED. Kept rather than re-read at
        /// submit: a file that changed or was deleted in between would other-
        /// wise vanish from the prompt while the bar still showed it, and the
        /// model would answer about nothing with no warning. It also keeps the
        /// submit path off a second synchronous read of every file.
        let text: String
        /// Characters actually readable from the file, after the read cap.
        /// Derived, not stored: two fields to keep in sync would let the
        /// context-budget warning report a size the prompt no longer carries.
        var characters: Int { text.count }
        /// The file was longer than `TextExtraction` reads.
        let truncated: Bool

        var id: String { path }
        var name: String { (path as NSString).lastPathComponent }
    }

    var isEmpty: Bool { files.isEmpty }
    var paths: [String] { files.map(\.path) }
    var totalCharacters: Int { files.reduce(0) { $0 + $1.characters } }

    /// Roughly how much of a model's window the attachments will occupy.
    var estimatedTokens: Int { totalCharacters / AIGenerationOptions.charactersPerToken }

    /// True once the attachments alone approach the window, where the model
    /// starts losing either them or the question. Takes the CONFIGURED
    /// provider's window: a cloud model's is far larger than a local one's, so
    /// a shared constant here would warn at the wrong point for both.
    func exceedsContext(_ contextTokens: Int) -> Bool {
        estimatedTokens > contextTokens * 3 / 4
    }

    /// Reads the file to learn its real size, so the bar reports what the model
    /// will get rather than what the file claims on disk. Returns the failure
    /// for the caller to show; a file that cannot be read is never attached.
    mutating func add(path: String) -> TextExtraction.Failure? {
        if files.contains(where: { $0.path == path }) { return nil }
        switch TextExtraction.extract(path: path) {
        case .success(let extracted):
            files.append(
                Attached(
                    path: path,
                    text: extracted.text,
                    truncated: extracted.truncated))
            return nil
        case .failure(let failure):
            return failure
        }
    }

    mutating func remove(path: String) {
        files.removeAll { $0.path == path }
    }

    mutating func removeAll() {
        files.removeAll()
    }

    /// The attachments as model context: one block per file, each labeled with
    /// its path so the answer can cite which file it came from. Built from the
    /// text captured at attach time, so what the model reads is exactly what
    /// the bar says it is carrying.
    func contextBlock() -> String {
        files
            .map { "--- \($0.path)\n\($0.text)" }
            .joined(separator: "\n\n")
    }
}
