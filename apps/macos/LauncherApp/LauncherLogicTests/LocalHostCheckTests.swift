import XCTest
@testable import LauncherLogic

/// The gate deciding whether private context (calendar, clipboard, remembered
/// facts) may ride along with a prompt. A false positive here ships personal
/// data off the machine, so the suspicious lookalikes matter more than the
/// happy path.
final class LocalHostCheckTests: XCTestCase {
    func testLoopbackFormsAreLocal() {
        for host in [
            "http://localhost:11434",
            "http://127.0.0.1:11434",
            "https://localhost",
            "localhost:11434",
            "127.0.0.1",
            "http://127.1.2.3:11434",
            "http://[::1]:11434",
            "  http://LOCALHOST:11434  ",
        ] {
            XCTAssertTrue(LocalHostCheck.isLocal(host: host), host)
        }
    }

    func testLookalikesAreNotLocal() {
        for host in [
            "http://localhost.evil.com:11434",
            "http://notlocalhost",
            "http://127.0.0.1.evil.com",
            "http://mylocalhost",
            "http://user@localhost",
            "http://localhost@evil.com",
        ] {
            XCTAssertFalse(LocalHostCheck.isLocal(host: host), host)
        }
    }

    func testRemoteHostsAreNotLocal() {
        for host in [
            "http://192.168.1.50:11434",
            "http://ollama.internal:11434",
            "https://api.openai.com",
            "http://10.0.0.5",
            "",
            "   ",
        ] {
            XCTAssertFalse(LocalHostCheck.isLocal(host: host), host)
        }
    }
}
