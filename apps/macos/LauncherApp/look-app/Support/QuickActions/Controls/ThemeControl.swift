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
        return await MainActor.run { Self.setDarkMode(target) }
    }

    private func isDark() -> Bool {
        UserDefaults.standard.string(forKey: Self.interfaceStyleKey)?.lowercased() == Self.darkStyleValue
    }

    @MainActor
    private static func setDarkMode(_ dark: Bool) -> ActionOutcome {
        let source = """
        tell application "System Events"
            tell appearance preferences
                set dark mode to \(dark ? "true" : "false")
            end tell
        end tell
        """
        return AppleScriptRunner.run(
            source,
            successBanner: dark ? "Dark theme" : "Light theme",
            failureMessage: "Could not switch theme"
        )
    }
}
