import Foundation
import Combine

final class AppUIState: ObservableObject {
    @Published var showsThemeSettings = false
}

extension Notification.Name {
    static let lookReloadConfigRequested = Notification.Name("look.reloadConfigRequested")
    static let lookRefocusInputRequested = Notification.Name("look.refocusInputRequested")
    static let lookFocusSettingsInputRequested = Notification.Name("look.focusSettingsInputRequested")
    static let lookToggleWindowRequested = Notification.Name("look.toggleWindowRequested")
    static let lookActivateLauncherRequested = Notification.Name("look.activateLauncherRequested")
    static let lookHideLauncherRequested = Notification.Name("look.hideLauncherRequested")
    static let lookToggleClipboardRequested = Notification.Name("look.toggleClipboardRequested")
}
