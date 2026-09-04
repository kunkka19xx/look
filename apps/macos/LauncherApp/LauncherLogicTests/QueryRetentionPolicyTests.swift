import XCTest

@testable import LauncherLogic

/// The `query_retention_seconds` rule, which has to agree with
/// the linows parser for the same key.
final class QueryRetentionPolicyTests: XCTestCase {
    private let key = AppConstants.Launcher.QueryRetention.configKey
    private let disabled = AppConstants.Launcher.QueryRetention.disabled

    func testAMissingKeyKeepsTheQuery() {
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [:]), disabled)
        XCTAssertEqual(
            QueryRetentionPolicy.resolveSeconds(from: ["clipboard_history_limit": "10"]),
            disabled)
    }

    func testMinusOneAndValuesAtOrAboveTheMinimumAreAccepted() {
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: "-1"]), -1)
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: "5"]), 5)
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: "12"]), 12)
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: " 7 "]), 7)
    }

    /// A too-eager value falls back to "never clear" rather than being clamped up
    /// to the minimum: silently clearing at 5s for someone who asked for 1s is a
    /// behavior they did not choose.
    func testUnparseableAndTooSmallValuesFallBack() {
        for raw in ["-2", "0", "3", "4", "abc", "1.5", ""] {
            XCTAssertEqual(
                QueryRetentionPolicy.resolveSeconds(from: [key: raw]), disabled,
                "raw=\(raw)")
        }
    }

    func testAHideShorterThanTheTimeoutKeepsTheQuery() {
        let hiddenAt = Date()
        XCTAssertFalse(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: 5, now: hiddenAt.addingTimeInterval(4.999)))
    }

    func testTheBoundaryAndBeyondClear() {
        let hiddenAt = Date()
        XCTAssertTrue(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: 5, now: hiddenAt.addingTimeInterval(5)))
        XCTAssertTrue(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: 5, now: hiddenAt.addingTimeInterval(3600)))
    }

    func testMinusOneNeverClears() {
        let hiddenAt = Date()
        XCTAssertFalse(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: -1, now: hiddenAt.addingTimeInterval(86_400)))
    }

    func testAnOpenWithNoRecordedHideNeverClears() {
        XCTAssertFalse(QueryRetentionPolicy.shouldClear(hiddenAt: nil, seconds: 5))
    }

    /// A clock moved backwards under a hidden launcher yields a negative
    /// interval; preserving the query is the safe answer.
    func testAClockMovedBackwardsKeepsTheQuery() {
        let hiddenAt = Date()
        XCTAssertFalse(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: 5, now: hiddenAt.addingTimeInterval(-600)))
    }
}
