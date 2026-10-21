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

    func testCloudRoutedModelsAreRemoteEvenOnLocalhost() {
        // Ollama proxies these through the local daemon to its hosted service,
        // so the loopback host proves nothing about where inference happens.
        for model in ["gpt-oss:120b-cloud", "qwen3-coder:480b-cloud", "some-model:cloud"] {
            XCTAssertTrue(LocalHostCheck.isCloudModel(model), model)
            XCTAssertFalse(
                LocalHostCheck.isLocalInference(host: "http://localhost:11434", model: model),
                model)
        }
    }

    func testOrdinaryModelsOnLocalhostStayLocal() {
        for model in ["llama3.1", "qwen2.5-coder:7b", "mistral:latest", ""] {
            XCTAssertFalse(LocalHostCheck.isCloudModel(model), model)
            XCTAssertTrue(
                LocalHostCheck.isLocalInference(host: "http://localhost:11434", model: model),
                model)
        }
    }

    func testRemoteHostsAreNotLocal() {
        for host in [
            // The unspecified address means "listen on every interface", which
            // is the opposite of proof that inference stays on this machine.
            "http://0.0.0.0:11434",
            "0.0.0.0",
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
