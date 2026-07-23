import Foundation
import OSLog

/// Runs AppleScript for the controls that drive System Events (appearance,
/// restart, shut down) and read audio settings (mic). It is the single place
/// scripts execute, so logging and the Automation-permission mapping live once.
///
/// NSAppleScript is not thread-safe and executing it blocks the calling thread,
/// so every script runs serialized on a private queue (never the main thread);
/// callers `await` the result.
enum AppleScriptRunner {
    private static let log = Logger(subsystem: "noah-code.Look", category: "actions.applescript")
    /// `errAEEventNotPermitted`: the user hasn't granted Automation access yet.
    private static let notPermittedCode = -1743
    private static let queue = DispatchQueue(label: "noah-code.Look.applescript")

    /// Runs a side-effect script, mapping the result to an `ActionOutcome`:
    /// `.ok(successBanner)` on success, `.needsPermission(...)` when Automation
    /// consent is missing, `.failed(failureMessage)` on any other error
    /// (including a script that fails to compile).
    static func run(_ source: String, successBanner: String?, failureMessage: String) async -> ActionOutcome {
        await withCheckedContinuation { continuation in
            queue.async {
                continuation.resume(returning: execute(source, successBanner: successBanner, failureMessage: failureMessage))
            }
        }
    }

    /// Evaluates a script and returns its integer result, or nil when the script
    /// fails to compile or run.
    static func evaluateInt(_ source: String) async -> Int? {
        await withCheckedContinuation { continuation in
            queue.async {
                guard let script = NSAppleScript(source: source) else {
                    log.error("AppleScript failed to compile")
                    continuation.resume(returning: nil)
                    return
                }
                var error: NSDictionary?
                let result = script.executeAndReturnError(&error)
                if let error {
                    log.error("AppleScript failed: \(error, privacy: .public)")
                    continuation.resume(returning: nil)
                    return
                }
                continuation.resume(returning: Int(result.int32Value))
            }
        }
    }

    /// Runs a script for its side effect only, returning whether it succeeded.
    static func runSucceeds(_ source: String) async -> Bool {
        await withCheckedContinuation { continuation in
            queue.async {
                guard let script = NSAppleScript(source: source) else {
                    log.error("AppleScript failed to compile")
                    continuation.resume(returning: false)
                    return
                }
                var error: NSDictionary?
                script.executeAndReturnError(&error)
                if let error {
                    log.error("AppleScript failed: \(error, privacy: .public)")
                }
                continuation.resume(returning: error == nil)
            }
        }
    }

    private static func execute(_ source: String, successBanner: String?, failureMessage: String) -> ActionOutcome {
        guard let script = NSAppleScript(source: source) else {
            log.error("AppleScript failed to compile")
            return .failed(failureMessage)
        }
        var error: NSDictionary?
        script.executeAndReturnError(&error)
        guard let error else {
            return .ok(banner: successBanner)
        }
        log.error("AppleScript failed: \(error, privacy: .public)")
        let code = (error[NSAppleScript.errorNumber] as? Int) ?? 0
        if code == notPermittedCode {
            return .needsPermission(
                "Allow Look under System Settings > Privacy & Security > Automation"
            )
        }
        return .failed(failureMessage)
    }
}
