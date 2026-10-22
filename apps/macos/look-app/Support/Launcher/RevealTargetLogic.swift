import Foundation

/// What revealing a path should do, decided without AppKit so it is testable.
enum RevealTargetLogic {
    enum Plan: Equatable {
        case selectInFileViewer
        case openURL
        case unavailable
    }

    static func plan(for path: String, exists: Bool) -> Plan {
        guard !path.isEmpty else { return .unavailable }
        guard !DeleteTargetLogic.isURLScheme(path) else { return .openURL }
        return exists ? .selectInFileViewer : .unavailable
    }
}
