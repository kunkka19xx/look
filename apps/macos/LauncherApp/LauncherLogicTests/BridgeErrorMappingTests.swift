import XCTest
@testable import LauncherLogic

final class BridgeErrorMappingTests: XCTestCase {
    func testKnownUsageErrorCodeMapsToFriendlyMessage() {
        let message = BridgeErrorMapping.userFacingMessage(
            code: BridgeErrorCode.invalidUsageAction.rawValue,
            fallback: "internal"
        )
        XCTAssertEqual(message, "This item could not be tracked.")
    }

    func testKnownTranslateInputErrorMapsToFriendlyMessage() {
        let message = BridgeErrorMapping.userFacingMessage(
            code: BridgeErrorCode.emptyText.rawValue,
            fallback: "internal"
        )
        XCTAssertEqual(message, "Type some text to continue.")
    }

    func testUnknownCodeFallsBackToProvidedMessage() {
        let message = BridgeErrorMapping.userFacingMessage(
            code: "totally_unknown_code",
            fallback: "Keep this fallback"
        )
        XCTAssertEqual(message, "Keep this fallback")
    }
}
