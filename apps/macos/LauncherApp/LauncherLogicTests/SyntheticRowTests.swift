import XCTest
@testable import LauncherLogic

/// Synthesized rows are identified purely by an id prefix, and `classify`
/// decides what Enter does. A miss here is silent and wrong: the AI action row
/// would fall through to "open a file at path ''" instead of running the
/// action, so the round-trip and the disjointness are worth pinning.
final class SyntheticRowTests: XCTestCase {
    func testAIActionRoundTripsItsToolID() {
        let id = AppConstants.Launcher.AIAction.resultID(toolID: "calendar.add_event")
        guard case .aiAction(let toolID)? = SyntheticRow.classify(resultID: id) else {
            return XCTFail("expected aiAction, got \(String(describing: SyntheticRow.classify(resultID: id)))")
        }
        XCTAssertEqual(toolID, "calendar.add_event")
    }

    func testPrefixesStayDisjoint() {
        // Each synthesized kind must classify as itself and nothing else.
        let cases: [(String, String)] = [
            (AppConstants.Launcher.AIAction.resultID(toolID: "reminder.add"), "aiAction"),
            ("\(AppConstants.Launcher.WebSuggestion.resultIDPrefix)coffee", "webSuggestion"),
            ("\(AppConstants.Launcher.PrefixSuggestion.resultIDPrefix)f\"", "prefixSuggestion"),
            ("\(AppConstants.Launcher.Calc.resultIDPrefix)42", "calc"),
            ("\(AppConstants.Launcher.CommandSuggestion.resultIDPrefix)calc", "commandSuggestion"),
            (
                AppConstants.Launcher.Meeting.resultID(url: "https://meet.jit.si/standup"),
                "meeting"
            ),
        ]
        for (id, expected) in cases {
            XCTAssertEqual(name(of: SyntheticRow.classify(resultID: id)), expected, id)
        }
    }

    func testRealCandidateIDsAreNotSynthetic() {
        // A real file/app id must never be mistaken for a synthesized row.
        for id in ["file:/Users/me/notes.txt", "app:safari", "folder:/tmp", "setting:display"] {
            XCTAssertNil(SyntheticRow.classify(resultID: id), id)
        }
    }

    private func name(of row: SyntheticRow?) -> String {
        switch row {
        case .aiAction: "aiAction"
        case .webSuggestion: "webSuggestion"
        case .prefixSuggestion: "prefixSuggestion"
        case .commandSuggestion: "commandSuggestion"
        case .webURL: "webURL"
        case .calc: "calc"
        case .meeting: "meeting"
        case nil: "nil"
        }
    }
}
