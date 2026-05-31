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
        case AppConstants.Launcher.Command.pomo:
            return "timer"
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
            static let pomo = "pomo"
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
                QuickFolderDefinition(title: "Pictures", relativePath: "Pictures"),
                // macOS names this folder "Movies" (Windows calls the equivalent "Videos");
                // each platform's QuickFolder uses the OS-native folder name so typing what
                // the user sees in Finder/Explorer pins it.
                QuickFolderDefinition(title: "Movies", relativePath: "Movies"),
                QuickFolderDefinition(title: "Music", relativePath: "Music"),
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

        enum Panel {
            static let width: CGFloat = 860
            static let height: CGFloat = 580
        }

        enum RunningAppsStrip {
            static let iconSize: CGFloat = 34
            static let horizontalPadding: CGFloat = 6
            static let verticalPadding: CGFloat = 10
            static let itemGap: CGFloat = 8
            // Slack on each end of the strip to keep the active ring from being clipped.
            static let edgeSlack: CGFloat = 8
            static let maxItems = 9

            static var width: CGFloat { iconSize + horizontalPadding * 2 + edgeSlack }

            /// Cmd-number keys in order of physical ease to press from
            /// the typical Cmd-Space launcher posture: left index/middle
            /// fingers first (1, 2, 3), then right-hand edge (9, 8),
            /// then 4 and 7, and finally the painful centre keys
            /// (6, 5). Used as a *resource* — when the strip has fewer
            /// than 9 icons we only consume the easy keys from the
            /// front of this list, so 5/6/7 only appear in 7+ app
            /// configurations.
            private static let easinessOrder: [Int] = [1, 2, 3, 9, 8, 4, 7, 6, 5]

            /// Returns the Cmd-number keys to assign to a strip of
            /// `total` icons, in left-to-right visual order. We pick the
            /// `total` easiest keys from `easinessOrder` and sort them
            /// ascending so the strip still reads naturally (e.g. for
            /// total=5 → `[1, 2, 3, 8, 9]` instead of `[1, 2, 3, 9, 8]`).
            static func badgeKeys(total: Int) -> [Int] {
                guard total > 0 else { return [] }
                return Array(easinessOrder.prefix(min(total, maxItems))).sorted()
            }

            /// The Cmd-number key shown on the badge of the icon at
            /// `position` (left-to-right, 0-indexed) in a strip of size
            /// `total`. Returns `position + 1` as a fallback for any
            /// out-of-range query.
            static func ergonomicKey(forVisualPosition position: Int, total: Int) -> Int {
                let keys = badgeKeys(total: total)
                guard position >= 0, position < keys.count else { return position + 1 }
                return keys[position]
            }

            /// Inverse of `ergonomicKey`: maps the Cmd-number key the
            /// user pressed (1..9) to the visual position of the icon
            /// they targeted. Returns nil when that key isn't currently
            /// assigned to any icon (e.g. user pressed Cmd+5 with only
            /// 4 running apps).
            static func visualPosition(forKey key: Int, total: Int) -> Int? {
                badgeKeys(total: total).firstIndex(of: key)
            }
        }

        static let commandCatalog: [AppCommand] = [
            AppCommand(id: Command.calc, title: "calc (⌘1)", detail: "Evaluate math expression", placeholder: "Type math expression"),
            AppCommand(id: Command.pomo, title: "pomo (⌘2)", detail: "Pomodoro focus timer", placeholder: "Manage focus sessions"),
            AppCommand(id: Command.kill, title: "kill (⌘3)", detail: "Force kill app or process by port", placeholder: "Type app name, or :3000"),
            AppCommand(id: Command.shell, title: "shell (⌘4)", detail: "Run a shell command", placeholder: "Type shell command"),
            AppCommand(id: Command.sys, title: "sys (⌘5)", detail: "Show system information", placeholder: "View system info"),
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
