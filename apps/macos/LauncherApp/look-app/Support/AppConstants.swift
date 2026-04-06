import Foundation
import CoreGraphics

struct AppCommand: Identifiable {
    let id: String
    let title: String
    let detail: String
    let placeholder: String
}

enum AppConstants {
    enum Launcher {
        static let defaultSearchLimit = 40
        static let commandListMaxHeight: CGFloat = 180
        static let commandResultFontSize: CGFloat = 18
        static let calcMaxMagnitude = 1_000_000_000_000.0

        static let commandCatalog: [AppCommand] = [
            AppCommand(id: "shell", title: "shell (⌘1)", detail: "Run a shell command", placeholder: "Type shell command"),
            AppCommand(id: "calc", title: "calc (⌘2)", detail: "Evaluate math expression", placeholder: "Type math expression"),
            AppCommand(id: "kill", title: "kill (⌘3)", detail: "Force kill a running app", placeholder: "Type app name to kill"),
            AppCommand(id: "sys", title: "sys", detail: "Show system information", placeholder: "View system info"),
        ]

        static let normalHint = HintText.Launcher.normal
        static let commandHint = HintText.Launcher.command
        static let killHint = HintText.Launcher.kill
        static let sysHint = HintText.Launcher.sys
        static let clipboardHint = HintText.Launcher.clipboard
        static let commandEmptyMessage = "Type expression and press Enter"
    }

    enum ThemeUI {
        static let labelWidth: CGFloat = 110
        static let pickerWidth: CGFloat = 140
    }
}
