import XCTest
@testable import LauncherLogic

final class ProcessSearchLogicTests: XCTestCase {
    func testFuzzySubsequenceMatchesProcessName() {
        XCTAssertNotNil(
            ProcessSearchLogic.score(query: "vsc", name: "Visual Studio Code", pid: 42)
        )
        XCTAssertNotNil(
            ProcessSearchLogic.score(query: "visual studio", name: "Visual Studio Code", pid: 42)
        )
    }

    func testSearchIsCaseAndDiacriticInsensitive() {
        XCTAssertNotNil(
            ProcessSearchLogic.score(query: "dien", name: "Điện thoại", pid: 42)
        )
    }

    func testPIDCanBeSearchedDirectly() {
        XCTAssertNotNil(ProcessSearchLogic.score(query: "424", name: "Terminal", pid: 4_242))
        XCTAssertNil(ProcessSearchLogic.score(query: "999", name: "Terminal", pid: 4_242))
    }

    func testExactNameOutranksFuzzyName() {
        let exact = ProcessSearchLogic.score(query: "code", name: "Code", pid: 10)
        let fuzzy = ProcessSearchLogic.score(query: "code", name: "Visual Code", pid: 11)
        XCTAssertGreaterThan(exact!, fuzzy!)
    }
}
