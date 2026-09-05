import Foundation
@testable import LauncherLogic

/// The shared wire fixture, read the way the app reads the payload.
///
/// One loader for both launchpad suites: the path walk and the snake-case
/// decoder are the contract itself, and two copies could disagree about it.
enum LaunchpadFixture {
    static func json() throws -> Data { try fixture("launchpad_layout.json") }

    /// Its own fixture because the default has no user tile.
    static func customTileJSON() throws -> Data { try fixture("launchpad_custom_tile.json") }

    private static func fixture(_ name: String) throws -> Data {
        // Up from LauncherLogicTests/ to the repo root: LauncherApp, macos,
        // apps, then the root itself.
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        return try Data(
            contentsOf: root.appendingPathComponent("bridge/ffi/tests/fixtures/\(name)"))
    }

    static func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(type, from: data)
    }

    static func layout() throws -> LaunchpadLayout {
        try decode(LaunchpadLayout.self, from: json())
    }

    static func customTileLayout() throws -> LaunchpadLayout {
        try decode(LaunchpadLayout.self, from: customTileJSON())
    }
}
