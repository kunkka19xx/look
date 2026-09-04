import XCTest

@testable import LauncherLogic

/// The `query_retention_seconds` rule, which has to agree with the linows parser.
final class QueryRetentionPolicyTests: XCTestCase {
    private let key = AppConstants.Launcher.QueryRetention.configKey
    private let fallback = AppConstants.Launcher.QueryRetention.defaultSeconds
    private let never = AppConstants.Launcher.QueryRetention.never

    func testAMissingKeyFallsBackToClearing() {
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [:]), fallback)
        XCTAssertEqual(
            QueryRetentionPolicy.resolveSeconds(from: ["clipboard_history_limit": "10"]),
            fallback)
        XCTAssertNotEqual(fallback, never)
        XCTAssertGreaterThanOrEqual(fallback, AppConstants.Launcher.QueryRetention.minimumSeconds)
    }

    func testTheNeverSentinelAndValuesAtOrAboveTheMinimumAreAccepted() {
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: "-1"]), never)
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: "5"]), 5)
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: "12"]), 12)
        XCTAssertEqual(QueryRetentionPolicy.resolveSeconds(from: [key: " 7 "]), 7)
    }

    func testUnparseableAndTooSmallValuesFallBack() {
        for raw in ["-2", "0", "3", "4", "abc", "1.5", ""] {
            XCTAssertEqual(
                QueryRetentionPolicy.resolveSeconds(from: [key: raw]), fallback,
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

    func testTheNeverSentinelNeverClears() {
        let hiddenAt = Date()
        XCTAssertFalse(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: -1, now: hiddenAt.addingTimeInterval(86_400)))
    }

    func testAnOpenWithNoRecordedHideNeverClears() {
        XCTAssertFalse(QueryRetentionPolicy.shouldClear(hiddenAt: nil, seconds: 5))
    }

    func testAClockMovedBackwardsKeepsTheQuery() {
        let hiddenAt = Date()
        XCTAssertFalse(
            QueryRetentionPolicy.shouldClear(
                hiddenAt: hiddenAt, seconds: 5, now: hiddenAt.addingTimeInterval(-600)))
    }
}
