import XCTest

@testable import LauncherLogic

final class SettingsRowTests: XCTestCase {
    private func result(id: String, path: String) -> LauncherResult {
        LauncherResult(id: id, kind: .app, title: "Wi-Fi", subtitle: nil, path: path, score: 0)
    }

    /// Panes are indexed as apps because that is how they open, so the badge
    /// can only tell them apart by the id prefix.
    func testAPaneIsASettingsRow() {
        let pane = result(
            id: "setting:com.apple.wifi-settings-extension",
            path: "x-apple.systempreferences:com.apple.wifi-settings-extension"
        )
        XCTAssertTrue(pane.isSettingsRow)
    }

    /// System Settings itself is a real app and keeps the App badge.
    func testTheSettingsAppIsNotASettingsRow() {
        let app = result(
            id: "app:/System/Applications/System Settings.app",
            path: "/System/Applications/System Settings.app"
        )
        XCTAssertFalse(app.isSettingsRow)
    }
}
