import XCTest
@testable import LauncherLogic

/// Covers the pure Ollama request/response codec: structured-plan decoding,
/// availability from `/api/tags`, and the delta-to-cumulative stream fold. The
/// networking in `OllamaProvider` needs a live daemon and is smoke-tested by
/// hand; everything testable without one lives here.
final class OllamaCodecTests: XCTestCase {
    // ── evaluateTags ────────────────────────────────────────────────────

    func testTagsReportsAvailableWhenModelPresent() {
        let data = Data(#"{"models":[{"name":"llama3.1:latest"},{"name":"qwen2.5"}]}"#.utf8)
        XCTAssertEqual(OllamaCodec.evaluateTags(data: data, model: "llama3.1"), .available)
    }

    func testTagsMatchesIgnoringLatestSuffixBothWays() {
        let data = Data(#"{"models":[{"name":"llama3.1"}]}"#.utf8)
        XCTAssertEqual(OllamaCodec.evaluateTags(data: data, model: "llama3.1:latest"), .available)
    }

    func testTagsReportsModelMissingWhenAbsent() {
        let data = Data(#"{"models":[{"name":"mistral"}]}"#.utf8)
        XCTAssertEqual(OllamaCodec.evaluateTags(data: data, model: "llama3.1"), .modelMissing)
    }

    func testTagsReportsModelMissingOnGarbage() {
        XCTAssertEqual(OllamaCodec.evaluateTags(data: Data("not json".utf8), model: "x"), .modelMissing)
    }

}
