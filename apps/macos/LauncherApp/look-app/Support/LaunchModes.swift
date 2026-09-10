import AppKit
import Foundation

@_silgen_name("look_modes_list_text")
nonisolated
private func look_modes_list_text() -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_modes_parse_json")
nonisolated
private func look_modes_parse_json(_ argvJSON: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_free_cstring")
nonisolated
private func look_free_cstring(_ ptr: UnsafeMutablePointer<CChar>?)

/// `lookapp clipboard`: open the launcher already in a mode. The table and the
/// argv grammar live in core (`core/engine/modes.rs`); this carries the answer
/// across and nothing else.
enum LaunchModes {
    /// Scoped to this bundle id so `lookapp` drives the installed app and
    /// `lookdev` drives Look Dev, rather than whichever answers first.
    static var deliveryNotification: Notification.Name {
        Notification.Name("look.launchQueryDelivered.\(bundleID)")
    }

    /// A query this process will serve itself, applied once the launcher is up.
    nonisolated(unsafe) static var pendingQuery: String?

    /// An exit code when the process has said its piece and should stop before
    /// SwiftUI starts, or nil to keep launching.
    static func handleLaunchArguments() -> Int32? {
        switch parse(Array(CommandLine.arguments.dropFirst())) {
        case .normal:
            return nil

        case .listModes:
            print(listText(), terminator: "")
            return 0

        case .unknownMode(let name):
            FileHandle.standardError.write(
                Data("lookapp: unknown mode \"\(name)\"\n\n\(listText())".utf8))
            return 2

        case .query(let text):
            guard isSameAppAlreadyRunning() else {
                pendingQuery = text
                return nil
            }
            DistributedNotificationCenter.default().postNotificationName(
                deliveryNotification, object: text, userInfo: nil, deliverImmediately: true)
            return 0
        }
    }

    private enum Launch {
        case normal
        case query(String)
        case listModes
        case unknownMode(String)
    }

    private struct Decision: Decodable {
        let kind: String
        let text: String?
        let name: String?
    }

    private static var bundleID: String {
        Bundle.main.bundleIdentifier ?? "unknown"
    }

    private static func parse(_ arguments: [String]) -> Launch {
        guard
            let argv = try? JSONEncoder().encode(arguments),
            let decision = call(look_modes_parse_json, with: String(decoding: argv, as: UTF8.self)),
            let decoded = try? JSONDecoder().decode(Decision.self, from: Data(decision.utf8))
        else {
            return .normal
        }

        switch decoded.kind {
        case "query": return decoded.text.map(Launch.query) ?? .normal
        case "list_modes": return .listModes
        case "unknown_mode": return .unknownMode(decoded.name ?? "")
        default: return .normal
        }
    }

    private static func listText() -> String {
        guard let ptr = look_modes_list_text() else { return "" }
        defer { look_free_cstring(ptr) }
        return String(cString: ptr)
    }

    private static func call(
        _ function: (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?, with argument: String
    ) -> String? {
        guard let ptr = argument.withCString({ function($0) }) else { return nil }
        defer { look_free_cstring(ptr) }
        return String(cString: ptr)
    }

    private static func isSameAppAlreadyRunning() -> Bool {
        let current = NSRunningApplication.current.processIdentifier
        return NSWorkspace.shared.runningApplications.contains {
            $0.bundleIdentifier == bundleID && $0.processIdentifier != current
        }
    }
}
