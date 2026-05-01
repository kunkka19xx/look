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
    // visited. Used so re-entering command mode (Cmd+/) reopens the
    // panel they last had focused, instead of always defaulting to
    // /calc. nil = no preference yet → fall back to /calc.
    @Published var lastCommandID: String? {
        didSet {
            UserDefaults.standard.set(lastCommandID, forKey: Self.lastCommandIDKey)
        }
    }

    private static let settingsBlurMultiplierKey = "look.ui.settingsBlurMultiplier"
    private static let lastCommandIDKey = "look.ui.lastCommandID"

    init() {
        if let stored = UserDefaults.standard.object(forKey: Self.settingsBlurMultiplierKey) as? Double,
            stored > 0
        {
            settingsBlurMultiplier = min(max(stored, 0.4), 1.0)
        } else {
            settingsBlurMultiplier = 0.5
        }
        lastCommandID = UserDefaults.standard.string(forKey: Self.lastCommandIDKey)
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
