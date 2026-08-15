import Foundation

/// Files attached to the next AI turn with `@`. Ordered, deduped, budgeted -
/// Ollama truncates a too-long prompt silently, so the bar says what it holds.
nonisolated struct MentionAttachments: Equatable {

    private(set) var files: [Attached] = []

    struct Attached: Equatable, Identifiable {
        let path: String
        /// Read at ATTACH time. Re-reading at submit would let a file that
        /// changed since vanish from the prompt while the bar still showed it.
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

    /// Takes the configured provider's window: a cloud model's is far larger
    /// than a local one's.
    func exceedsContext(_ contextTokens: Int) -> Bool {
        estimatedTokens > contextTokens * 3 / 4
    }

    /// Returns the failure for the caller to show; an unreadable file is never
    /// attached.
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
