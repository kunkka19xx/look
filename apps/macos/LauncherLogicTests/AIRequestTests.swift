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

    /// A provider that takes one string still needs to know who said what;
    /// otherwise a restored conversation reads as one giant user question.
    func testHistoryKeepsWhoSaidWhat() {
        let messages = [
            AIMessage(.system, "Be brief."),
            AIMessage(.user, "What is 2+2?"),
            AIMessage(.assistant, "4"),
            AIMessage(.user, "And 3+3?"),
        ]
        let prompt = AIMessage.conversation(messages)
        XCTAssertEqual(prompt, "User: What is 2+2?\n\nAssistant: 4\n\nUser: And 3+3?")
        // System messages stay out; they are the provider's instructions.
        XCTAssertFalse(prompt.contains("Be brief."))
    }

    func testASingleQuestionIsNotLabelled() {
        let one = [AIMessage(.system, "Be brief."), AIMessage(.user, "What is 2+2?")]
        XCTAssertEqual(AIMessage.conversation(one), "What is 2+2?")
    }

    /// A blank turn must not make a single question look like history: the
    /// count has to be taken AFTER empties are dropped, not before.
    func testABlankTurnDoesNotTurnOneQuestionIntoHistory() {
        let withBlank = [
            AIMessage(.system, "Be brief."),
            AIMessage(.user, "What is 2+2?"),
            AIMessage(.assistant, "   "),
        ]
        XCTAssertEqual(AIMessage.conversation(withBlank), "What is 2+2?")

        // Real history is still labelled, blank turns and all.
        let mixed = [
            AIMessage(.user, "What is 2+2?"),
            AIMessage(.assistant, ""),
            AIMessage(.assistant, "4"),
        ]
        XCTAssertEqual(AIMessage.conversation(mixed), "User: What is 2+2?\n\nAssistant: 4")
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
