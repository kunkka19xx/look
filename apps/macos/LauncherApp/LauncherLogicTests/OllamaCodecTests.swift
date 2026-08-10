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

    // ── decodePlan ──────────────────────────────────────────────────────

    func testDecodesPlanFromChatContent() {
        let data = Data(#"{"message":{"role":"assistant","content":"{\"kind\":\"app\",\"searchText\":\"spotify\"}"}}"#.utf8)
        XCTAssertEqual(OllamaCodec.decodePlan(fromChatResponse: data), OllamaCodec.Plan(kind: "app", searchText: "spotify"))
    }

    func testDecodePlanReturnsNilWhenContentNotJSON() {
        let data = Data(#"{"message":{"content":"sorry, I cannot help"}}"#.utf8)
        XCTAssertNil(OllamaCodec.decodePlan(fromChatResponse: data))
    }

    func testDecodePlanReturnsNilWhenMessageMissing() {
        XCTAssertNil(OllamaCodec.decodePlan(fromChatResponse: Data("{}".utf8)))
    }

    // ── parseStreamLine ─────────────────────────────────────────────────

    func testParseStreamLineExtractsDeltaAndDone() {
        let (delta, done, error) = OllamaCodec.parseStreamLine(#"{"message":{"content":"Hel"},"done":false}"#)!
        XCTAssertEqual(delta, "Hel")
        XCTAssertFalse(done)
        XCTAssertNil(error)
    }

    func testParseStreamLineSeesDoneTrue() {
        let (_, done, _) = OllamaCodec.parseStreamLine(#"{"message":{"content":""},"done":true}"#)!
        XCTAssertTrue(done)
    }

    func testParseStreamLineIgnoresBlankLine() {
        XCTAssertNil(OllamaCodec.parseStreamLine("   "))
    }

    func testParseStreamLineSurfacesServerError() {
        let (_, _, error) = OllamaCodec.parseStreamLine(#"{"error":"model 'x' not found"}"#)!
        XCTAssertEqual(error, "model 'x' not found")
    }

    func testErrorMessageFromResponseBody() {
        let body = Data(#"{"error":"model 'x' not found"}"#.utf8)
        XCTAssertEqual(OllamaCodec.errorMessage(fromResponseBody: body), "model 'x' not found")
        XCTAssertNil(OllamaCodec.errorMessage(fromResponseBody: Data("{}".utf8)))
        XCTAssertNil(OllamaCodec.errorMessage(fromResponseBody: Data("not json".utf8)))
    }

    // ── cumulativeSnapshots (the delta-accumulation gotcha) ──────────────

    func testCumulativeSnapshotsAccumulateDeltas() {
        let lines = [
            #"{"message":{"content":"Hel"},"done":false}"#,
            #"{"message":{"content":"lo "},"done":false}"#,
            #"{"message":{"content":"world"},"done":false}"#,
            #"{"message":{"content":""},"done":true}"#,
        ]
        XCTAssertEqual(
            OllamaCodec.cumulativeSnapshots(fromStreamLines: lines),
            ["Hel", "Hello ", "Hello world"]
        )
    }

    func testCumulativeSnapshotsStopAtDone() {
        let lines = [
            #"{"message":{"content":"A"},"done":false}"#,
            #"{"message":{"content":""},"done":true}"#,
            #"{"message":{"content":"ignored"},"done":false}"#,
        ]
        XCTAssertEqual(OllamaCodec.cumulativeSnapshots(fromStreamLines: lines), ["A"])
    }

    func testCumulativeSnapshotsStopAtServerError() {
        let lines = [
            #"{"message":{"content":"A"},"done":false}"#,
            #"{"error":"boom"}"#,
            #"{"message":{"content":"B"},"done":false}"#,
        ]
        XCTAssertEqual(OllamaCodec.cumulativeSnapshots(fromStreamLines: lines), ["A"])
    }

    // ── chatRequestBody ─────────────────────────────────────────────────

    func testRequestBodyIncludesFormatAndZeroTempForStructured() throws {
        let body = OllamaCodec.chatRequestBody(
            model: "llama3.1", system: "sys", user: "hi",
            stream: false, format: OllamaCodec.intentJSONSchema(), numPredict: 200
        )!
        let json = try XCTUnwrap(try JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["model"] as? String, "llama3.1")
        XCTAssertEqual(json["stream"] as? Bool, false)
        XCTAssertNotNil(json["format"])
        XCTAssertEqual((json["options"] as? [String: Any])?["temperature"] as? Double, 0)
        XCTAssertEqual((json["options"] as? [String: Any])?["num_predict"] as? Int, 200)
    }

    func testRequestBodyOmitsFormatForFreeform() throws {
        let body = OllamaCodec.chatRequestBody(
            model: "llama3.1", system: "sys", user: "hi",
            stream: true, format: nil, numPredict: 220
        )!
        let json = try XCTUnwrap(try JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertNil(json["format"])
        XCTAssertEqual(json["stream"] as? Bool, true)
    }
}
