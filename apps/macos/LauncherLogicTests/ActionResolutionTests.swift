import XCTest
@testable import LauncherLogic

/// The shell half of the resolve contract with `core/ai/src/resolve.rs`. This
/// decoder is the only thing standing between the core's JSON and executing a
/// calendar change, and a drift here fails silently (the action just stops
/// working), so the three outcomes and the optional-date handling are pinned.
final class ActionResolutionTests: XCTestCase {
    private func decode(_ json: String) -> ActionResolveOutcome {
        ActionResolveOutcome.decode(Data(json.utf8))
    }

    func testPlannedCarriesPreviewAndExecuteSpec() throws {
        let outcome = decode("""
        {"outcome":"planned","preview_title":"Add event",
         "preview_detail":"\\"Dentist\\"  Tue Aug 5, 10:00-11:00",
         "summary":"Added \\"Dentist\\"","subject":"new",
         "execute":{"kind":"add_event","title":"Dentist","start":1754388000,
                    "end":1754391600,"all_day":false},
         "undo":{"kind":"remove_event_by_new_id"}}
        """)
        guard case .planned(let plan) = outcome else {
            return XCTFail("expected planned, got \(outcome)")
        }
        XCTAssertEqual(plan.previewTitle, "Add event")
        XCTAssertEqual(plan.summary, "Added \"Dentist\"")
        XCTAssertEqual(plan.subject, "new")
        guard case .addEvent(let title, let start, _, let allDay) = plan.execute else {
            return XCTFail("expected addEvent, got \(plan.execute)")
        }
        XCTAssertEqual(title, "Dentist")
        XCTAssertEqual(start, Date(timeIntervalSince1970: 1_754_388_000))
        XCTAssertFalse(allDay)
    }

    func testChoiceCarriesCandidates() {
        let outcome = decode("""
        {"outcome":"choice","candidates":[{"id":"e1","label":"Sync  ·  Mon"},
                                          {"id":"e2","label":"Sync  ·  Fri"}]}
        """)
        guard case .choice(let candidates) = outcome else {
            return XCTFail("expected choice, got \(outcome)")
        }
        XCTAssertEqual(candidates.map(\.id), ["e1", "e2"])
        XCTAssertEqual(candidates.first?.label, "Sync  ·  Mon")
    }

    func testInvalidKeepsTheCoreMessage() {
        guard case .invalid(let message) = decode(
            #"{"outcome":"invalid","message":"Which event?"}"#)
        else { return XCTFail("expected invalid") }
        XCTAssertEqual(message, "Which event?")
    }

    func testUnreadableOutcomeDegradesInsteadOfCrashing() {
        // A shape the core never sends must still land somewhere safe.
        guard case .invalid = decode("not json") else {
            return XCTFail("garbage should decode to invalid")
        }
        guard case .invalid = decode(#"{"outcome":"planned"}"#) else {
            return XCTFail("planned without a plan should decode to invalid")
        }
    }

    func testNullDueDecodesAsUndatedReminder() throws {
        // An undated reminder sends `due: null`; treating that as a failure
        // would break every "remind me to X" with no time.
        let outcome = decode("""
        {"outcome":"planned","preview_title":"Add reminder","preview_detail":"x",
         "summary":"Added","subject":"new",
         "execute":{"kind":"add_reminder","title":"Call plumber","due":null},
         "undo":{"kind":"remove_reminder_by_new_id"}}
        """)
        guard case .planned(let plan) = outcome,
              case .addReminder(let title, let due) = plan.execute
        else { return XCTFail("expected planned addReminder, got \(outcome)") }
        XCTAssertEqual(title, "Call plumber")
        XCTAssertNil(due)
    }
}
