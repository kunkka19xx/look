import XCTest
@testable import LauncherLogic

final class SingleInstanceLockTests: XCTestCase {
    func testLockPathIsStableForSameBundlePath() {
        let bundlePath = "/Applications/Look.app"
        let first = SingleInstanceLock.lockPath(for: bundlePath, tempDirectory: "/tmp")
        let second = SingleInstanceLock.lockPath(for: bundlePath, tempDirectory: "/tmp")

        XCTAssertEqual(first, second)
    }

    func testSecondAcquireReportsHeldByOtherInstance() {
        let lockPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("look-single-instance-test-\(UUID().uuidString).lock")
            .path

        let firstResult = SingleInstanceLock.acquire(
            lockPath: lockPath,
            timeoutSeconds: 0.2,
            pollIntervalMicros: 10_000
        )

        guard case .acquired(let firstFD) = firstResult else {
            XCTFail("Expected first lock acquisition to succeed")
            return
        }

        defer {
            SingleInstanceLock.release(firstFD)
            try? FileManager.default.removeItem(atPath: lockPath)
        }

        let secondResult = SingleInstanceLock.acquire(
            lockPath: lockPath,
            timeoutSeconds: 0.05,
            pollIntervalMicros: 10_000
        )

        guard case .heldByOtherInstance = secondResult else {
            XCTFail("Expected second lock acquisition to report heldByOtherInstance")
            return
        }
    }

    func testAcquireWorksAfterRelease() {
        let lockPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("look-single-instance-test-\(UUID().uuidString).lock")
            .path

        let firstResult = SingleInstanceLock.acquire(
            lockPath: lockPath,
            timeoutSeconds: 0.2,
            pollIntervalMicros: 10_000
        )

        guard case .acquired(let firstFD) = firstResult else {
            XCTFail("Expected first lock acquisition to succeed")
            return
        }

        SingleInstanceLock.release(firstFD)

        let secondResult = SingleInstanceLock.acquire(
            lockPath: lockPath,
            timeoutSeconds: 0.2,
            pollIntervalMicros: 10_000
        )

        guard case .acquired(let secondFD) = secondResult else {
            XCTFail("Expected lock acquisition after release to succeed")
            return
        }

        SingleInstanceLock.release(secondFD)
        try? FileManager.default.removeItem(atPath: lockPath)
    }
}
