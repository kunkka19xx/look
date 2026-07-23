import Foundation

/// Toggles and reports the system appearance. "On" means Dark. Action id:
/// `"theme"`. The current setting is read from the global `AppleInterfaceStyle`
/// default; switching it drives System Events via AppleScript, which needs
/// Automation consent (prompted on first use, entitlement already declared).
struct ThemeControl: SystemControl {
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
        let source = """
        tell application "System Events"
            tell appearance preferences
                set dark mode to \(target ? "true" : "false")
            end tell
        end tell
        """
        return await AppleScriptRunner.run(
            source,
            successBanner: target ? "Dark theme" : "Light theme",
            failureMessage: "Could not switch theme"
        )
    }

    private func isDark() -> Bool {
        UserDefaults.standard.string(forKey: Self.interfaceStyleKey)?.lowercased() == Self.darkStyleValue
    }
}
