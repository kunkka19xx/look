import Foundation
import CoreGraphics

struct AppCommand: Identifiable {
    let id: String
    let title: String
    let detail: String
    let placeholder: String

    var symbolName: String {
        switch id {
        case AppConstants.Launcher.Command.shell:
            return "terminal"
        case AppConstants.Launcher.Command.calc:
            return "function"
        case AppConstants.Launcher.Command.kill:
            return "xmark.circle"
        case AppConstants.Launcher.Command.sys:
            return "info.circle"
        default:
            return "terminal"
        }
    }
}

struct QuickFolderDefinition {
    let title: String
    let relativePath: String
}

enum AppConstants {
    enum Launcher {
        enum Command {
            static let shell = "shell"
            static let calc = "calc"
            static let kill = "kill"
            static let sys = "sys"
        }

        enum QueryPrefix {
            static let apps = "a\""
            static let files = "f\""
            static let folders = "d\""
            static let regex = "r\""
            static let clipboard = "c\""
        }

        enum Finder {
            static let appName = "finder"
            static let appPath = "/System/Library/CoreServices/Finder.app"
            static var pinnedResultID: String {
                "app:\(appPath.lowercased())"
            }
            static let pinnedSubtitle = "Pinned system app"
            static let pinnedScore = 999_999
            static let minPrefixMatchLength = 3
            static let cannotRevealBanner = "Cannot reveal this target in Finder"
        }

        enum QuickFolder {
            static let idPrefix = "quickfolder:"
            static let pinnedSubtitle = "Pinned home folder"
            static let minPrefixMatchLength = 2
            static let entries: [QuickFolderDefinition] = [
                QuickFolderDefinition(title: "Desktop", relativePath: "Desktop"),
                QuickFolderDefinition(title: "Documents", relativePath: "Documents"),
                QuickFolderDefinition(title: "Downloads", relativePath: "Downloads"),
            ]
        }

        enum Clipboard {
            static let resultIDPrefix = "clipboard:"
            static let resultPath = "clipboard://history"
            static let maxEntries = 10
            static let maxStoredCharacters = 30_000
            static let foregroundPollInterval: TimeInterval = 0.35
            static let backgroundPollInterval: TimeInterval = 0.9
            static let burstPollInterval: TimeInterval = 0.08
            static let burstSampleCount = 10
            static let copiedBanner = "Copied clipboard item"
            static let deletedBanner = "Clipboard item deleted"
            static let nonFileBanner = "Clipboard items are not files"
            static let copiedBannerDuration = 1.2
            static let infoBannerDuration = 1.1
        }

        enum Help {
            static let commandModeInfoBanner = "Help is available in app list mode"
        }

        static let defaultSearchLimit = 40
        static let searchDebounceNanoseconds: UInt64 = 70_000_000
        static let windowCornerRadius: CGFloat = 16
        static let commandListMaxHeight: CGFloat = 180
        static let commandResultFontSize: CGFloat = 18
        static let calcMaxMagnitude = 1_000_000_000_000.0

        static let commandCatalog: [AppCommand] = [
            AppCommand(id: Command.shell, title: "shell (⌘1)", detail: "Run a shell command", placeholder: "Type shell command"),
            AppCommand(id: Command.calc, title: "calc (⌘2)", detail: "Evaluate math expression", placeholder: "Type math expression"),
            AppCommand(id: Command.kill, title: "kill (⌘3)", detail: "Force kill app or process by port", placeholder: "Type app name, or :3000"),
            AppCommand(id: Command.sys, title: "sys (⌘4)", detail: "Show system information", placeholder: "View system info"),
        ]

        static let normalHint = HintText.Launcher.normal
        static let commandHint = HintText.Launcher.command
        static let killHint = HintText.Launcher.kill
        static let sysHint = HintText.Launcher.sys
        static let commandEmptyMessage = "Type expression and press Enter"
    }

    enum ThemeUI {
        static let labelWidth: CGFloat = 150
        static let pickerWidth: CGFloat = 140
    }

    enum FileScan {
        static let minDepth = 1
        static let maxDepth = 12
        static let minLimit = 500
        static let maxLimit = 50_000
    }
}
