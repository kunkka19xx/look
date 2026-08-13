import XCTest

@testable import LauncherLogic

final class AIRequestTests: XCTestCase {
    func testFlatteningIsTheFallbackNotTheDefault() {
        let messages = [
            AIMessage(.system, "Translate to vietnamese."),
            AIMessage(.user, "hello"),
        ]
        XCTAssertEqual(AIMessage.flattened(messages), "Translate to vietnamese.\n\nhello")
        // Providers that take instructions separately get them separated.
        XCTAssertEqual(AIMessage.instructions(messages), "Translate to vietnamese.")
        XCTAssertEqual(AIMessage.conversation(messages), "hello")
    }

    func testEmptyMessagesAreDroppedFromTheFlattenedForm() {
        let messages = [AIMessage(.system, "   "), AIMessage(.user, "hi")]
        XCTAssertEqual(AIMessage.flattened(messages), "hi")
        XCTAssertEqual(AIMessage.instructions(messages), "   ")
    }

    func testOllamaTranslationOmitsContextUnlessItIsNeeded() {
        // A chat reply fits the daemon's default, so asking for a bigger KV
        // cache would cost memory on every request for nothing.
        let chat = AIGenerationOptions.chat.ollamaJSON(contextCeiling: 16384)
        XCTAssertTrue(chat.contains("\"num_predict\":512"))
        XCTAssertFalse(chat.contains("num_ctx"))
    }

    func testOllamaTranslationSizesTheWindowToTheDocument() {
        // ~8k tokens of prompt: too big for the 4096 default.
        let options = AIGenerationOptions.document(promptCharacters: 32_000)
        let json = options.ollamaJSON(contextCeiling: 16384)
        XCTAssertTrue(json.contains("\"num_predict\":2048"))
        XCTAssertTrue(json.contains("\"num_ctx\":16384"), json)

        // ~3k tokens: over the default once the answer is reserved, so the
        // window doubles once rather than jumping to the ceiling.
        let medium = AIGenerationOptions.document(promptCharacters: 12_000)
            .ollamaJSON(contextCeiling: 16384)
        XCTAssertTrue(medium.contains("\"num_ctx\":8192"), medium)

        // A small file plus its answer still fits 4096, so nothing is asked
        // for: reserving a window it does not need costs memory per request.
        let small = AIGenerationOptions.document(promptCharacters: 4_000)
            .ollamaJSON(contextCeiling: 16384)
        XCTAssertFalse(small.contains("num_ctx"), small)
    }

    func testTheCeilingIsRespected() {
        // A provider that will not reserve more than 4096 never gets asked to.
        let json = AIGenerationOptions.document(promptCharacters: 200_000)
            .ollamaJSON(contextCeiling: 4096)
        XCTAssertTrue(json.contains("\"num_ctx\":4096"), json)
    }

    func testAnswerCardStaysShortAndGivesUpFast() {
        let json = AIGenerationOptions.answerCard.ollamaJSON(contextCeiling: 16384)
        XCTAssertTrue(json.contains("\"num_predict\":220"))
        XCTAssertTrue(json.contains("\"timeout_secs\":45"))
    }
}
