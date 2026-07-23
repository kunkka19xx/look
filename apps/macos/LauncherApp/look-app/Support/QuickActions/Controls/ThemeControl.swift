import AppKit
import Foundation
import OSLog

/// Toggles and reports the system appearance. "On" means Dark. Action id:
/// `"theme"`. The current setting is read from the global `AppleInterfaceStyle`
/// default; switching it drives System Events via AppleScript, which needs
/// Automation consent (prompted on first use, entitlement already declared).
struct ThemeControl: SystemControl {
    private static let log = Logger(subsystem: "noah-code.Look", category: "actions.theme")
    /// `errAEEventNotPermitted`: the user hasn't granted Automation access yet.
    private static let notPermittedCode = -1743
    private static let interfaceStyleKey = "AppleInterfaceStyle"
    private static let darkStyleValue = "dark"

    func state() async -> ActionState {
        isDark() ? .on : .off
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        let target: Bool
        switch intent {
        case .toggle:
            target = !isDark()
        case .setOn(let on):
            target = on
        case .run:
            return .failed("Theme has no run action")
        }
        return await MainActor.run { Self.setDarkMode(target) }
    }

    private func isDark() -> Bool {
        UserDefaults.standard.string(forKey: Self.interfaceStyleKey)?.lowercased() == Self.darkStyleValue
    }

    /// Runs on the main actor: NSAppleScript is not thread-safe.
    @MainActor
    private static func setDarkMode(_ dark: Bool) -> ActionOutcome {
        let source = """
        tell application "System Events"
            tell appearance preferences
                set dark mode to \(dark ? "true" : "false")
            end tell
        end tell
        """
        var error: NSDictionary?
        NSAppleScript(source: source)?.executeAndReturnError(&error)
        if let error {
            log.error("theme apply failed: \(error, privacy: .public)")
            let code = (error[NSAppleScript.errorNumber] as? Int) ?? 0
            if code == notPermittedCode {
                return .needsPermission(
                    "Allow Look under System Settings > Privacy & Security > Automation"
                )
            }
            return .failed("Could not switch theme")
        }
        return .ok(banner: dark ? "Dark theme" : "Light theme")
    }
}
