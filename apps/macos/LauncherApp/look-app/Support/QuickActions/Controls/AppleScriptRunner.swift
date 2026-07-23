import Foundation
import OSLog

/// Runs an AppleScript and maps the result to an `ActionOutcome`, so every
/// control that drives System Events (appearance, restart, shut down) shares one
/// place for execution, logging, and the Automation-permission mapping.
enum AppleScriptRunner {
    private static let log = Logger(subsystem: "noah-code.Look", category: "actions.applescript")
    /// `errAEEventNotPermitted`: the user hasn't granted Automation access yet.
    private static let notPermittedCode = -1743

    /// Executes `source` on the main actor (NSAppleScript is not thread-safe).
    /// Returns `.ok(successBanner)` on success, `.needsPermission(...)` when
    /// Automation consent is missing, or `.failed(failureMessage)` otherwise.
    @MainActor
    static func run(_ source: String, successBanner: String?, failureMessage: String) -> ActionOutcome {
        var error: NSDictionary?
        NSAppleScript(source: source)?.executeAndReturnError(&error)
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
