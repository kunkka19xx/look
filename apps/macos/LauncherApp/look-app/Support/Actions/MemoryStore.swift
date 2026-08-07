import Foundation

/// Thin shell over the Rust-core long-term memory (core/ai/memory), which owns
/// the cap, dedup, and the `remember`/`forget` grammar. The shell supplies the
/// platform path: `~/Library/Application Support/Look/ai-memory.json`.
@MainActor
enum MemoryStore {
    private static var filePath: String? {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?
            .appendingPathComponent("Look", isDirectory: true)
            .appendingPathComponent("ai-memory.json")
            .path
    }

    /// Feedback to show when `input` is a memory command, else nil (normal input).
    static func command(_ input: String) -> String? {
        guard let path = filePath else { return nil }
        return EngineBridge.shared.aiMemoryCommand(path: path, input: input)
    }

    /// The stored facts as a model context block, empty when there are none.
    static func context() -> String {
        guard let path = filePath else { return "" }
        return EngineBridge.shared.aiMemoryContext(path: path)
    }
}
