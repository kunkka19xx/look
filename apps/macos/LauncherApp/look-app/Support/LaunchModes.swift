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

    static var reloadConfigNotification: Notification.Name {
        Notification.Name("look.reloadConfigRequested.\(bundleID)")
    }

    static var toggleNotification: Notification.Name {
        Notification.Name("look.launchToggleDelivered.\(bundleID)")
    }

    /// A query this process will serve itself, applied once the launcher is up.
    nonisolated(unsafe) static var pendingQuery: String?
    /// A cold `lookapp --toggle`: show the launcher once it is up.
    nonisolated(unsafe) static var pendingToggle = false

    /// Set on the app a cold `lookapp` launches, so it serves the mode itself
    /// instead of mistaking that still-exiting `lookapp` for a running Look.
    private static let handoffEnvironmentKey = "LOOK_LAUNCH_HANDOFF"
    private static let handoffEnvironmentValue = "1"
    private static let handoffTimeoutSeconds: TimeInterval = 10

    /// An exit code when the process has said its piece and should stop before
    /// SwiftUI starts, or nil to keep launching.
    static func handleLaunchArguments() -> Int32? {
        switch parse(Array(CommandLine.arguments.dropFirst())) {
        case .normal:
            return AppBundle.isLaunchedOutsideBundle ? handOff() : nil

        case .listModes:
            print(listText(), terminator: "")
            return 0

        case .reloadConfig:
            // Headless by contract: with no instance up there is nothing to
            // reload, and the next launch reads the file anyway.
            guard isSameAppAlreadyRunning() else {
                FileHandle.standardError.write(
                    Data("lookapp: Look is not running, config will load on next launch\n".utf8))
                return 0
            }
            DistributedNotificationCenter.default().postNotificationName(
                reloadConfigNotification, object: nil, userInfo: nil, deliverImmediately: true)
            return 0

        case .unknownMode(let name):
            FileHandle.standardError.write(
                Data("lookapp: unknown mode \"\(name)\"\n\n\(listText())".utf8))
            return 2

        case .unavailableMode(let name):
            FileHandle.standardError.write(
                Data("lookapp: mode \"\(name)\" is not available on this platform\n".utf8))
            return 2

        case .query(let text):
            guard isSameAppAlreadyRunning() else {
                guard consumeHandoffMarker() else { return handOff() }
                pendingQuery = text
                return nil
            }
            DistributedNotificationCenter.default().postNotificationName(
                deliveryNotification, object: text, userInfo: nil, deliverImmediately: true)
            return 0

        case .toggle:
            guard isSameAppAlreadyRunning() else {
                guard consumeHandoffMarker() else { return handOff() }
                pendingToggle = true
                return nil
            }
            DistributedNotificationCenter.default().postNotificationName(
                toggleNotification, object: nil, userInfo: nil, deliverImmediately: true)
            return 0
        }
    }

    private enum Launch {
        case normal
        case query(String)
        case toggle
        case listModes
        case reloadConfig
        case unknownMode(String)
        case unavailableMode(String)
    }

    private struct Decision: Decodable {
        let kind: String
        let text: String?
        let name: String?
    }

    private static var bundleID: String {
        AppBundle.identifier ?? "unknown"
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
        case "reload_config": return .reloadConfig
        case "toggle": return .toggle
        case "unknown_mode": return .unknownMode(decoded.name ?? "")
        case "unavailable_mode": return .unavailableMode(decoded.name ?? "")
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

    /// Starts Look.app through Launch Services with this invocation's arguments
    /// and returns the CLI's exit code. The CLI never becomes the app itself: run
    /// through a symlink it has no bundle, and from a shell it would tie Look's
    /// lifetime to the terminal and block whatever ran it.
    private static func handOff() -> Int32 {
        guard let appURL = AppBundle.url else {
            FileHandle.standardError.write(Data("lookapp: cannot locate Look.app\n".utf8))
            return 1
        }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.arguments = Array(CommandLine.arguments.dropFirst())
        configuration.environment = [handoffEnvironmentKey: handoffEnvironmentValue]

        let outcome = HandoffOutcome()
        let finished = DispatchSemaphore(value: 0)
        NSWorkspace.shared.openApplication(at: appURL, configuration: configuration) { _, error in
            outcome.error = error
            finished.signal()
        }
        guard finished.wait(timeout: .now() + handoffTimeoutSeconds) == .success else {
            FileHandle.standardError.write(Data("lookapp: timed out launching Look\n".utf8))
            return 1
        }
        if let error = outcome.error {
            FileHandle.standardError.write(Data("lookapp: \(error.localizedDescription)\n".utf8))
            return 1
        }
        return 0
    }

    private static func consumeHandoffMarker() -> Bool {
        guard ProcessInfo.processInfo.environment[handoffEnvironmentKey] == handoffEnvironmentValue else {
            return false
        }
        unsetenv(handoffEnvironmentKey)
        return true
    }

    private static func isSameAppAlreadyRunning() -> Bool {
        let current = NSRunningApplication.current.processIdentifier
        return NSWorkspace.shared.runningApplications.contains {
            $0.bundleIdentifier == bundleID && $0.processIdentifier != current
        }
    }
}

nonisolated private final class HandoffOutcome: @unchecked Sendable {
    var error: Error?
}
