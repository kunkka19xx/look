import Foundation
import Combine

final class AppUIState: ObservableObject {
    @Published var showsThemeSettings = false
    @Published var settingsBlurMultiplier: Double {
        didSet {
            UserDefaults.standard.set(settingsBlurMultiplier, forKey: Self.settingsBlurMultiplierKey)
        }
    }

    // Remembered command id of the last command-mode panel the user
    // visited *during this launch*. Re-entering command mode (Cmd+/)
    // resumes there instead of jumping back to /calc. Intentionally
    // not persisted — each fresh launch should start at /calc.
    @Published var lastCommandID: String?

    private static let settingsBlurMultiplierKey = "look.ui.settingsBlurMultiplier"

    init() {
        if let stored = UserDefaults.standard.object(forKey: Self.settingsBlurMultiplierKey) as? Double,
            stored > 0
        {
            settingsBlurMultiplier = min(max(stored, 0.4), 1.0)
        } else {
            settingsBlurMultiplier = 0.5
        }
    }
}

extension Notification.Name {
    static let lookReloadConfigRequested = Notification.Name("look.reloadConfigRequested")
    static let lookRefocusInputRequested = Notification.Name("look.refocusInputRequested")
    static let lookFocusSettingsInputRequested = Notification.Name("look.focusSettingsInputRequested")
    static let lookToggleWindowRequested = Notification.Name("look.toggleWindowRequested")
    static let lookActivateLauncherRequested = Notification.Name("look.activateLauncherRequested")
    static let lookHideLauncherRequested = Notification.Name("look.hideLauncherRequested")
}
