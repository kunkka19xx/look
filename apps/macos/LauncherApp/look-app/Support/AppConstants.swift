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
    /// Where a quick folder lives. Most are under the user's home directory
    /// (`.home("Desktop")`); a few are fixed system locations outside home
    /// (`.absolute("/Applications")`).
    enum Location {
        case home(String)
        case absolute(String)
    }

    let title: String
    let location: Location
    var subtitle: String? = nil

    /// Resolves to a concrete filesystem path. Home-relative entries are joined
    /// onto `homeDirectory`; absolute entries are used verbatim.
    func resolvedPath(homeDirectory: String) -> String {
        switch location {
        case .home(let relativePath):
            return URL(fileURLWithPath: homeDirectory)
                .appendingPathComponent(relativePath)
                .path
        case .absolute(let path):
            return path
        }
    }
}

enum AppConstants {
    enum Launcher {
        /// Search field placeholder shown in normal (non-command) mode.
        static let searchPlaceholder = "Type whatever you want"
        /// Search field placeholder shown in command mode when no command is active.
        static let commandModePlaceholder = "Choose a command with Tab"

        /// Width of the web-suggestion column shown to the right of the AI answer
        /// card in the two-column knowledge-lookup layout.
        static let aiAnswerSuggestionColumnWidth: CGFloat = 320

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
            // Recent files/folders, newest-activity first. Handled engine-side
            // (needs last_used/fs_modified timestamps); the app just sends it
            // through search and suppresses pinned injection (see LauncherSearchLogic).
            static let recent = "rc\""
            // Translation prefixes (handled in LauncherView+Translation).
            static let translate = "t\""
            static let translateWord = "tw\""

            // Typing a lone `"` opens the prefix-discovery menu (see
            // PrefixSuggestion.all / LauncherView.isPrefixSuggestionQuery).
            static let discovery = "\""
        }

        // Canonical list of query prefixes, with a usage hint and a description.
        // Single source of truth for the prefix-discovery menu (type `"`), the help
        // screen's "Query modes" section, and the Settings → Shortcuts panel, so
        // the three can't drift apart.
        enum PrefixSuggestion {
            // Synthetic result id prefix; lets the row view and open handler tell a
            // discovery suggestion apart from a real candidate.
            static let resultIDPrefix = "prefixhint:"

            struct Entry: Identifiable {
                let prefix: String
                let argHint: String
                let description: String
                /// Whether this entry appears in the live `"` discovery menu.
                /// The `"` entry itself is documented in the static lists but
                /// hidden from the menu it opens (see `menuEntries`).
                var listedInMenu: Bool = true
                var id: String { prefix }
                /// What the help/shortcuts lists show, e.g. `a"word` (or just `"`).
                var displayWithArg: String { prefix + argHint }
            }

            /// Entries shown in the live discovery menu (excludes `"` itself).
            static var menuEntries: [Entry] { all.filter(\.listedInMenu) }

            /// Discovery entries narrowed by `filter` - the text typed after the
            /// leading `"`. Case-insensitive substring match against the prefix,
            /// its display form, and the description, so `"folder` finds `d"` by
            /// intent rather than only by the cryptic prefix letter. An empty
            /// filter returns the full menu.
            static func menuEntries(matching filter: String) -> [Entry] {
                let needle = filter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                guard !needle.isEmpty else { return menuEntries }
                return menuEntries.filter {
                    $0.prefix.lowercased().contains(needle)
                        || $0.displayWithArg.lowercased().contains(needle)
                        || $0.description.lowercased().contains(needle)
                }
            }

            static let all: [Entry] = [
                Entry(
                    prefix: QueryPrefix.discovery, argHint: "",
                    description: "Browse all prefixes", listedInMenu: false),
                Entry(prefix: QueryPrefix.apps, argHint: "word", description: "Apps only"),
                Entry(prefix: QueryPrefix.files, argHint: "word", description: "Files only"),
                Entry(prefix: QueryPrefix.folders, argHint: "word", description: "Folders only"),
                Entry(
                    prefix: QueryPrefix.recent, argHint: "word",
                    description: "Recent files/folders, newest first (optional filter)"),
                Entry(prefix: QueryPrefix.regex, argHint: "pattern", description: "Regex search"),
                Entry(
                    prefix: QueryPrefix.clipboard, argHint: "word",
                    description: "Clipboard history search (latest 10 text clips)"),
                Entry(prefix: QueryPrefix.translate, argHint: "word", description: "Web translate (VI/EN/JA)"),
                Entry(
                    prefix: QueryPrefix.translateWord, argHint: "word",
                    description: "Lookup panel with definitions"),
            ]

            /// Recovers the query prefix encoded in a discovery-suggestion result
            /// id, or nil when `resultID` isn't a discovery suggestion.
            static func prefix(fromResultID resultID: String) -> String? {
                guard resultID.hasPrefix(resultIDPrefix) else { return nil }
                return String(resultID.dropFirst(resultIDPrefix.count))
            }
        }

        // Google autocomplete rows appended after the engine results. Like
        // PrefixSuggestion, these are Swift-synthesized rows told apart by id.
        enum WebSuggestion {
            static let resultIDPrefix = "websuggest:"
            static let limit = 6

            /// Recovers the suggestion text encoded in a result id, or nil.
            static func text(fromResultID resultID: String) -> String? {
                guard resultID.hasPrefix(resultIDPrefix) else { return nil }
                return String(resultID.dropFirst(resultIDPrefix.count))
            }
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
            static let absoluteFolderSubtitle = "Pinned folder"
            static let entries: [QuickFolderDefinition] = [
                QuickFolderDefinition(title: "Desktop", location: .home("Desktop")),
                QuickFolderDefinition(title: "Documents", location: .home("Documents")),
                QuickFolderDefinition(title: "Downloads", location: .home("Downloads")),
                QuickFolderDefinition(title: "Pictures", location: .home("Pictures")),
                // macOS names this folder "Movies" (Windows calls the equivalent "Videos");
                // each platform's QuickFolder uses the OS-native folder name so typing what
                // the user sees in Finder/Explorer pins it.
                QuickFolderDefinition(title: "Movies", location: .home("Movies")),
                QuickFolderDefinition(title: "Music", location: .home("Music")),
                // /Applications is a system folder outside home — the folder indexer
                // only walks Desktop/Documents/Downloads, so pin it here to make it
                // reachable. .app bundles inside it stay app candidates, not folders.
                QuickFolderDefinition(
                    title: "Applications",
                    location: .absolute("/Applications"),
                    subtitle: absoluteFolderSubtitle
                ),
                // ~/.Trash is a real directory, so it opens in Finder like any
                // other quick folder. Typing "trash" pins it; ⌘D empties it.
                QuickFolderDefinition(
                    title: "Trash", location: .home(".Trash"), subtitle: "Pinned · ⌘D to empty"),
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

        /// Commands narrowed by `filter` - the text typed after a leading `:`.
        /// Case-insensitive substring match against the command id and its
        /// description, so `:end` or `:process` both surface `kill`. An empty
        /// filter returns the whole catalog.
        static func commandCatalog(matching filter: String) -> [AppCommand] {
            let needle = filter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            guard !needle.isEmpty else { return commandCatalog }
            return commandCatalog.filter {
                $0.id.lowercased().contains(needle)
                    || $0.detail.lowercased().contains(needle)
            }
        }

        // Command-discovery rows (type `:`). Like PrefixSuggestion, these are
        // Swift-synthesized rows in the main results list, told apart by id;
        // `openSelectedApp` enters the command instead of opening a file.
        enum CommandSuggestion {
            static let resultIDPrefix = "cmdhint:"

            /// Recovers the command id encoded in a discovery-row result id, or nil.
            static func commandID(fromResultID resultID: String) -> String? {
                guard resultID.hasPrefix(resultIDPrefix) else { return nil }
                return String(resultID.dropFirst(resultIDPrefix.count))
            }
        }

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
