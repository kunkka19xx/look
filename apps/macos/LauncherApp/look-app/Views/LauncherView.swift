import AppKit
import CoreServices
import OSLog
import SwiftUI

struct LauncherView: View {
    private enum TranslationCommand {
        case network(String)
        case lookup(String)
    }

    private enum BannerStyle {
        case success
        case error
        case info
        case warning

        var background: Color {
            switch self {
            case .success:
                return .green.opacity(0.42)
            case .error:
                return .red.opacity(0.45)
            case .info:
                return .blue.opacity(0.40)
            case .warning:
                return .orange.opacity(0.45)
            }
        }
    }

    @EnvironmentObject private var appUIState: AppUIState
    @EnvironmentObject private var themeStore: ThemeStore
    @Environment(\.openWindow) private var openWindow
    @StateObject private var clipboardStore = ClipboardHistoryStore()

    @State private var query = ""
    @State private var commandInput = ""
    @State private var isCommandMode = false
    @State private var backendResults: [LauncherResult] = []
    @State private var selectedResultID: String?
    @State private var selectedCommandID: String?
    @State private var activeCommandID: String?
    @State private var commandFeedback = ""
    @State private var keyboardMonitor = KeyboardSelectionMonitor()
    @State private var searchTask: Task<Void, Never>?
    @State private var latestSearchID: UInt64 = 0
    @State private var bannerMessage: String?
    @State private var bannerStyle: BannerStyle = .info
    @State private var bannerCopyText: String?
    @State private var bannerTask: Task<Void, Never>?
    @State private var lookupPreviewTask: Task<Void, Never>?
    @State private var selectedKillSuggestionIndex: Int?
    @State private var pendingKillCandidate: KillCommand.Candidate?
    @State private var killListRefreshTick: Int = 0
    @State private var recentlyKilledPIDs: Set<Int32> = []
    @State private var showsHelpScreen = false
    @State private var focusRequestToken: UInt64 = 0
    @State private var lookupDefinition: LookupDefinition?
    @State private var pidToRestoreOnHide: pid_t?

    private static let postHideActivationDelay: TimeInterval = 0.01
    private static let postOpenActivationDelay: TimeInterval = 0.05
    @FocusState private var isQueryFocused: Bool

    private let bridge = EngineBridge.shared
    private let shouldShowTestHint = LauncherView.cachedShouldShowTestHint

    private static let cachedShouldShowTestHint: Bool = {
        let env = ProcessInfo.processInfo.environment
        if let value = env["LOOK_DEV_HINT"]?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
            ["1", "true", "yes", "on"].contains(value)
        {
            return true
        }

        if let configPath = env["LOOK_CONFIG_PATH"]?.trimmingCharacters(in: .whitespacesAndNewlines),
            configPath.lowercased().contains(".look.dev.config")
        {
            return true
        }

        if let bundleIdentifier = Bundle.main.bundleIdentifier,
            bundleIdentifier.caseInsensitiveCompare("noah-code.Look") != .orderedSame
        {
            return true
        }

        let bundlePath = Bundle.main.bundleURL.resolvingSymlinksInPath().path.lowercased()
        if bundlePath.contains("/look dev.app") {
            return true
        }

        return false
    }()

    private static let debugEventLoggingEnabled: Bool = {
        let env = ProcessInfo.processInfo.environment
        let raw = env["LOOK_UI_DEBUG_EVENTS"] ?? env["LOOK_DEV_HINT"] ?? ""
        return ["1", "true", "yes", "on"].contains(raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    }()

    private static let logger = Logger(subsystem: "noah-code.Look", category: "ui")

    private func logUIEvent(_ message: String) {
        guard Self.debugEventLoggingEnabled else { return }
        Self.logger.notice("\(message, privacy: .public)")
    }

    private static let clipboardSubtitleDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()

    private let commandCatalog: [AppCommand] = AppConstants.Launcher.commandCatalog

    private var pinnedLookupScope: LauncherPinnedLookupScope {
        LauncherSearchLogic.pinnedLookupScope(for: query)
    }

    private var normalizedPinnedLookupQuery: String? {
        LauncherSearchLogic.normalizedPinnedLookupQuery(for: query, scope: pinnedLookupScope)
    }

    private var shouldInjectFinderResult: Bool {
        LauncherSearchLogic.shouldInjectFinder(
            normalizedQuery: normalizedPinnedLookupQuery,
            scope: pinnedLookupScope
        )
    }

    private var quickFolderPinnedResults: [LauncherResult] {
        guard pinnedLookupScope == .unscoped || pinnedLookupScope == .folders else { return [] }
        guard let normalized = normalizedPinnedLookupQuery else { return [] }

        return AppConstants.Launcher.QuickFolder.entries.compactMap { entry in
            let normalizedTitle = entry.title.lowercased()
            let isMatch = normalizedTitle.contains(normalized)
                || (normalizedTitle.hasPrefix(normalized)
                    && normalized.count >= AppConstants.Launcher.QuickFolder.minPrefixMatchLength)
            guard isMatch else { return nil }

            let folderPath = URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent(entry.relativePath)
                .path
            guard FileManager.default.fileExists(atPath: folderPath) else { return nil }

            return LauncherResult(
                id: "\(AppConstants.Launcher.QuickFolder.idPrefix)\(normalizedTitle)",
                kind: .folder,
                title: entry.title,
                subtitle: AppConstants.Launcher.QuickFolder.pinnedSubtitle,
                path: folderPath,
                score: AppConstants.Launcher.Finder.pinnedScore
            )
        }
    }

    private var finderPinnedResult: LauncherResult {
        LauncherResult(
            id: AppConstants.Launcher.Finder.pinnedResultID,
            kind: .app,
            title: "Finder",
            subtitle: AppConstants.Launcher.Finder.pinnedSubtitle,
            path: AppConstants.Launcher.Finder.appPath,
            score: AppConstants.Launcher.Finder.pinnedScore
        )
    }

    private var backendFilteredResults: [LauncherResult] {
        var sourceResults = backendResults

        for quickFolder in quickFolderPinnedResults.reversed() {
            let alreadyPresent = sourceResults.contains { item in
                item.kind == .folder && item.path == quickFolder.path
            }
            if !alreadyPresent {
                sourceResults.insert(quickFolder, at: 0)
            }
        }

        if shouldInjectFinderResult {
            let hasFinder = sourceResults.contains {
                $0.title.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == AppConstants.Launcher.Finder.appName
                    || $0.path == AppConstants.Launcher.Finder.appPath
            }
            if !hasFinder {
                sourceResults.insert(finderPinnedResult, at: 0)
            }
        }

        return LauncherSearchLogic.dedupe(results: sourceResults)
    }

    private var isClipboardQuery: Bool {
        LauncherClipboardFeature.isClipboardQuery(query)
    }

    private var clipboardSearchTerm: String? {
        LauncherClipboardFeature.searchTerm(from: query)
    }

    private var clipboardResults: [LauncherResult] {
        guard let clipboardSearchTerm else { return [] }

        return clipboardStore.search(clipboardSearchTerm).map { entry in
            LauncherClipboardFeature.makeResult(entry: entry, dateFormatter: Self.clipboardSubtitleDateFormatter)
        }
    }

    private var displayedResults: [LauncherResult] {
        isClipboardQuery ? clipboardResults : backendFilteredResults
    }

    private var isTranslationQuery: Bool {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        return extractTranslationQuery(from: trimmed) != nil
    }

    private var translationEmptyHint: String? {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let command = extractTranslationQuery(from: trimmed) else {
            return nil
        }

        switch command {
        case .network:
            return "Press Enter after finishing input to translate on web"
        case .lookup:
            return nil
        }
    }

    private var isWebTranslationQuery: Bool {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let command = extractTranslationQuery(from: trimmed) else {
            return false
        }

        if case .network = command {
            return true
        }
        return false
    }

    private var currentHint: String {
        hintItems.joined(separator: "  •  ")
    }

    private var hintItems: [String] {
        if appUIState.showsThemeSettings {
            return [
                "Cmd+H help",
                "Cmd+/ command mode",
                "Cmd+Shift+, close settings",
                "Cmd+Shift+; apply config",
            ]
        }

        if isCommandMode {
            if activeCommandID == AppConstants.Launcher.Command.kill {
                return ["Y confirm", "N cancel", "Tab/Cmd+1-4 switch", "Esc back"]
            }
            if activeCommandID == AppConstants.Launcher.Command.sys {
                return ["Esc back", "Tab/Cmd+1-4 switch", "Cmd+/ command mode", "Cmd+Shift+, settings"]
            }
            return ["Enter run", "Tab select", "Cmd+1-4 switch", "Esc back"]
        }

        if let command = extractTranslationQuery(from: query.trimmingCharacters(in: .whitespacesAndNewlines)) {
            switch command {
            case .network:
                return ["Enter translate web", "Copy per result", "Cmd+H help", "Cmd+/ command mode"]
            case .lookup:
                return ["Live lookup", "Type to refine", "Cmd+H help", "Cmd+/ command mode"]
            }
        }

        if showsHelpScreen {
            return ["Cmd+H close help", "Esc hide launcher", "Cmd+/ command mode", "Enter open"]
        }

        if isClipboardQuery {
            return ["Enter copy clip", "Delete remove clip", "Cmd+H help", "Cmd+/ command mode"]
        }

        return ["Enter open", "Cmd+F reveal", "Cmd+H help", "Cmd+/ command mode"]
    }

    private var commandNamePart: String {
        guard activeCommandID == nil else { return "" }
        let normalized = commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return "" }
        return normalized.split(maxSplits: 1, whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? ""
    }

    private var commandArgsPart: String {
        if activeCommandID != nil {
            return commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
        }

        let normalized = commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let splitPoint = normalized.firstIndex(where: { $0.isWhitespace }) else { return "" }
        return String(normalized[splitPoint...]).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var activeCommand: AppCommand? {
        guard let activeCommandID else { return nil }
        return commandCatalog.first(where: { $0.id == activeCommandID })
    }

    private var activeCommandAcceptsInput: Bool {
        guard let activeCommandID else { return false }
        return activeCommandID != AppConstants.Launcher.Command.sys
    }

    private var isKillConfirmationVisible: Bool {
        isCommandMode
            && activeCommandID == AppConstants.Launcher.Command.kill
            && pendingKillCandidate != nil
    }

    private var liveCommandPreview: String? {
        guard isCommandMode else { return nil }

        if hasSudoWarning {
            return "Warning: sudo command detected"
        }

        if activeCommandID == AppConstants.Launcher.Command.calc {
            let expr = commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !expr.isEmpty else { return nil }
            guard CalcCommand.isReadyForEvaluation(expr) else { return nil }

            switch CalcCommand.evaluate(expr) {
            case .value(let value):
                return "Result: \(value)"
            case .error(let message):
                return message
            }
        }

        return nil
    }

    private var hasSudoWarning: Bool {
        guard isCommandMode, activeCommandID == AppConstants.Launcher.Command.shell else { return false }
        return ShellCommand.hasSudoWarning(commandInput)
    }

    private var filteredCommands: [AppCommand] {
        let prefix = commandNamePart.lowercased()
        if prefix.isEmpty {
            return commandCatalog
        }
        return commandCatalog.filter { $0.id.hasPrefix(prefix) }
    }

    private var killSuggestions: [KillCommand.Candidate] {
        _ = killListRefreshTick
        let searchTerm = commandArgsPart.trimmingCharacters(in: .whitespacesAndNewlines)
        return KillCommand.suggestions(searchTerm: searchTerm)
            .filter { !recentlyKilledPIDs.contains($0.pid) }
    }

    private func scheduleKillListRefresh() {
        killListRefreshTick &+= 1
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
            killListRefreshTick &+= 1
            if activeCommandID == AppConstants.Launcher.Command.kill {
                selectedKillSuggestionIndex = killSuggestions.first?.number
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            killListRefreshTick &+= 1
            if activeCommandID == AppConstants.Launcher.Command.kill {
                selectedKillSuggestionIndex = killSuggestions.first?.number
            }
        }
    }

    private func setInitialSelection() {
        if isCommandMode {
            if let activeCommandID {
                selectedCommandID = activeCommandID
            } else {
                selectedCommandID = filteredCommands.first?.id
            }
        } else {
            selectedResultID = displayedResults.first?.id
        }
    }

    private func moveSelection(
        _ direction: MoveCommandDirection,
        shouldAutocompleteCommand: Bool = false,
        preferCommandListInCommandMode: Bool = false
    ) {
        guard !appUIState.showsThemeSettings else { return }

        if isCommandMode
            && activeCommandID == AppConstants.Launcher.Command.kill
            && !preferCommandListInCommandMode
        {
            let suggestions = killSuggestions.prefix(20)
            guard !suggestions.isEmpty else { return }

            let currentNum = selectedKillSuggestionIndex
            let currentIndex = suggestions.firstIndex { $0.number == currentNum }

            let nextIndex: Int
            switch direction {
            case .down:
                if let currentIndex {
                    nextIndex = min(currentIndex + 1, suggestions.count - 1)
                } else {
                    nextIndex = 0
                }
            case .up:
                if let currentIndex {
                    nextIndex = max(currentIndex - 1, 0)
                } else {
                    nextIndex = suggestions.count - 1
                }
            default:
                return
            }

            selectedKillSuggestionIndex = suggestions[nextIndex].number
            return
        }

        if isCommandMode {
            guard !filteredCommands.isEmpty else {
                selectedCommandID = nil
                return
            }

            guard let currentID = selectedCommandID,
                let currentIndex = filteredCommands.firstIndex(where: { $0.id == currentID })
            else {
                selectedCommandID = filteredCommands.first?.id
                if shouldAutocompleteCommand {
                    autocompleteSelectedCommand()
                }
                return
            }

            let nextIndex: Int
            switch direction {
            case .down:
                nextIndex = (currentIndex + 1) % filteredCommands.count
            case .up:
                nextIndex = (currentIndex - 1 + filteredCommands.count) % filteredCommands.count
            default:
                return
            }

            selectedCommandID = filteredCommands[nextIndex].id
            if shouldAutocompleteCommand {
                autocompleteSelectedCommand()
            }
            return
        }

        guard !displayedResults.isEmpty else {
            selectedResultID = nil
            return
        }

        guard let currentID = selectedResultID,
            let currentIndex = displayedResults.firstIndex(where: { $0.id == currentID })
        else {
            selectedResultID = displayedResults.first?.id
            return
        }

        let nextIndex: Int
        switch direction {
        case .down:
            nextIndex = (currentIndex + 1) % displayedResults.count
        case .up:
            nextIndex = (currentIndex - 1 + displayedResults.count) % displayedResults.count
        default:
            return
        }

        selectedResultID = displayedResults[nextIndex].id
    }

    private func autocompleteSelectedCommand() {
        guard isCommandMode,
            let commandID = selectedCommandID,
            filteredCommands.contains(where: { $0.id == commandID })
        else { return }

        activeCommandID = commandID
        commandFeedback = "Selected /\(commandID)"

        requestCommandInputFocusIfNeeded()
    }

    private func requestCommandInputFocusIfNeeded() {
        guard isCommandMode else { return }
        guard activeCommandAcceptsInput else {
            isQueryFocused = false
            return
        }
        DispatchQueue.main.async {
            guard isCommandMode, activeCommandAcceptsInput else { return }
            focusActiveInput(recoveryDelays: [0.0], activateApp: false)
        }
    }

    private func enterCommandMode() {
        showsHelpScreen = false
        isCommandMode = true
        commandInput = ""
        commandFeedback = ""
        activeCommandID = AppConstants.Launcher.Command.calc
        selectedCommandID = AppConstants.Launcher.Command.calc
        focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
    }

    private func exitCommandMode() {
        guard isCommandMode else { return }
        isCommandMode = false
        commandInput = ""
        commandFeedback = ""
        activeCommandID = nil
        selectedCommandID = nil
        refreshSearchResults()
        focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
    }

    private func handleSubmit() {
        logUIEvent("submit isCommand=\(isCommandMode) active=\(activeCommandID ?? "nil") selectedKill=\(selectedKillSuggestionIndex.map(String.init) ?? "nil") pendingKill=\(pendingKillCandidate?.displayName ?? "nil") input='\(commandArgsPart)'")
        if isCommandMode {
            if activeCommandID == AppConstants.Launcher.Command.kill, let selectedNum = selectedKillSuggestionIndex {
                if let candidate = killSuggestions.first(where: { $0.number == selectedNum }) {
                    pendingKillCandidate = candidate
                    logUIEvent("kill submit -> pending from selected index num=\(selectedNum) candidate=\(candidate.displayName) pid=\(candidate.pid)")
                } else {
                    selectedKillSuggestionIndex = nil
                    logUIEvent("kill submit -> stale selection num=\(selectedNum), fallback action")
                    runCommandModeAction()
                }
            } else {
                runCommandModeAction()
            }
        } else {
            let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
            if let translationCommand = extractTranslationQuery(from: trimmed) {
                handleTranslation(command: translationCommand)
                isQueryFocused = true
            } else {
                openSelectedApp()
            }
        }

        DispatchQueue.main.async {
            isQueryFocused = true
        }
    }

    private func extractTranslationQuery(from input: String) -> TranslationCommand? {
        if input.hasPrefix("t\"") {
            let text = String(input.dropFirst(2)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .network(text)
        }

        if input.count >= 3, input.prefix(3).lowercased() == "tw\"" {
            let text = String(input.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .lookup(text)
        }

        if input.lowercased().hasPrefix("tr ") {
            let text = String(input.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .network(text)
        }

        return nil
    }

    private func handleTranslation(command: TranslationCommand) {
        switch command {
        case .network(let text):
            handleNetworkTranslation(text: text)
        case .lookup(let text):
            handleLookupTranslation(text: text)
        }
    }

    private func runCommandModeAction() {
        let resolvedCommand = commandCatalog.first(where: { $0.id == (activeCommandID ?? "") })
            ?? commandCatalog.first(where: { $0.id == commandNamePart.lowercased() })
            ?? commandCatalog.first(where: { $0.id == selectedCommandID })

        guard let resolvedCommand else {
            setCommandError("Unknown command. Try /shell, /calc, /kill, or /sys")
            return
        }

        switch resolvedCommand.id {
        case AppConstants.Launcher.Command.shell:
            guard !commandArgsPart.isEmpty else {
                setCommandError("Usage: /shell <command>")
                return
            }
            commandFeedback = "Running..."
            ShellCommand.run(commandArgsPart) { [self] message in
                commandFeedback = message
                isQueryFocused = true
            }
        case AppConstants.Launcher.Command.calc:
            guard !commandArgsPart.isEmpty else {
                setCommandError("Usage: /calc <expression>")
                return
            }
            let result = CalcCommand.evaluate(commandArgsPart)
            switch result {
            case .value(let value):
                commandFeedback = "Result: \(value)"
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(value, forType: .string)
            case .error(let message):
                setCommandError(message)
            }
        case AppConstants.Launcher.Command.kill:
            let searchTerm = commandArgsPart.trimmingCharacters(in: .whitespacesAndNewlines)
            let matched = KillCommand.suggestions(searchTerm: searchTerm)
            logUIEvent("kill action search='\(searchTerm)' matches=\(matched.count)")

            if matched.isEmpty {
                if searchTerm.hasPrefix(":") || searchTerm.lowercased().hasPrefix("port ") {
                    commandFeedback = "No process listening on this port"
                } else {
                    commandFeedback = "No matching apps. /kill to list all. Use :3000 to search by port."
                }
            } else if searchTerm.isEmpty {
                let appList = matched.map { candidate in
                    "\(candidate.number). \(candidate.displayName) (PID: \(candidate.pid))"
                }
                commandFeedback = "Running apps:\n" + appList.joined(separator: "\n") + "\n\n/kill <name or number>"
            } else if matched.count > 1 {
                let list = matched.map { candidate in "\(candidate.number). \(candidate.displayName)" }
                commandFeedback = "Multiple matches:\n" + list.joined(separator: "\n") + "\n\nBe more specific."
            } else {
                let candidate = matched[0]
                selectedKillSuggestionIndex = candidate.number
                pendingKillCandidate = candidate
                logUIEvent("kill action -> pending single candidate=\(candidate.displayName) pid=\(candidate.pid)")
            }
        case AppConstants.Launcher.Command.sys:
            commandFeedback = ""
        default:
            setCommandError("Unsupported command")
        }
    }

    private func setCommandError(_ message: String) {
        commandFeedback = message
        showBanner(message)
    }

    private func runKillCommand(candidate: KillCommand.Candidate) {
        logUIEvent("kill execute candidate=\(candidate.displayName) pid=\(candidate.pid)")
        KillCommand.kill(pid: candidate.pid, name: candidate.displayName) { [self] message in
            commandFeedback = message
            logUIEvent("kill completion message='\(message)'")
            pendingKillCandidate = nil
            selectedKillSuggestionIndex = nil

            if message.hasPrefix("Killed:") {
                recentlyKilledPIDs.insert(candidate.pid)
                DispatchQueue.main.asyncAfter(deadline: .now() + 10.0) {
                    recentlyKilledPIDs.remove(candidate.pid)
                }
            }
            scheduleKillListRefresh()
        }
    }

    private func openSelectedApp() {
        guard let selectedResultID,
            let selected = displayedResults.first(where: { $0.id == selectedResultID })
        else { return }

        switch selected.kind {
        case .app:
            if openTarget(selected.path) {
                bringOpenedAppToFront(appBundlePath: selected.path)
                if let error = bridge.recordUsage(candidateID: selected.id, action: "open_app") {
                    showBanner(error.userFacingMessage, style: .info, duration: 1.4)
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        case .file:
            if openTarget(selected.path) {
                if let error = bridge.recordUsage(candidateID: selected.id, action: "open_file") {
                    showBanner(error.userFacingMessage, style: .info, duration: 1.4)
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        case .folder:
            if openTarget(selected.path) {
                if !selected.id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix),
                    let error = bridge.recordUsage(candidateID: selected.id, action: "open_folder")
                {
                    showBanner(error.userFacingMessage, style: .info, duration: 1.4)
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        case .clipboard:
            guard let content = selected.clipboardContent, !content.isEmpty else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(content, forType: .string)
            showBanner(
                AppConstants.Launcher.Clipboard.copiedBanner,
                style: .success,
                duration: AppConstants.Launcher.Clipboard.copiedBannerDuration
            )
        }
    }

    @discardableResult
    private func openTarget(_ target: String) -> Bool {
        if target.contains(":") && !target.hasPrefix("/") {
            if let url = URL(string: target) {
                if NSWorkspace.shared.open(url) {
                    return true
                }
                showBanner("Could not open this item right now", style: .error, duration: 1.2)
                return false
            }
            showBanner("Invalid target URL", style: .error, duration: 1.2)
            return false
        }

        if NSWorkspace.shared.open(URL(fileURLWithPath: target)) {
            return true
        }

        showBanner("Could not open this path", style: .error, duration: 1.2)
        return false
    }

    private func revealSelectedInFinder() {
        guard !isCommandMode,
              let selectedID = selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedID })
        else { return }

        switch selected.kind {
        case .app, .file, .folder:
            if selected.path.contains(":") && !selected.path.hasPrefix("/") {
                if let url = URL(string: selected.path) {
                    NSWorkspace.shared.open(url)
                } else {
                    showBanner(
                        AppConstants.Launcher.Finder.cannotRevealBanner,
                        style: .info,
                        duration: AppConstants.Launcher.Clipboard.infoBannerDuration
                    )
                }
            } else {
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: selected.path)])
            }
        case .clipboard:
            showBanner(
                AppConstants.Launcher.Clipboard.nonFileBanner,
                style: .info,
                duration: AppConstants.Launcher.Clipboard.infoBannerDuration
            )
        }
    }

    private func copySelectedResultToPasteboard() -> Bool {
        guard !isCommandMode,
              let selectedID = selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedID })
        else { return false }

        guard selected.kind == .file || selected.kind == .folder else { return false }

        let targetURL = URL(fileURLWithPath: selected.path)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let didWrite = pasteboard.writeObjects([targetURL as NSURL, selected.path as NSString])

        if didWrite {
            showBanner("Copied \(selected.kind.rawValue) to pasteboard", style: .success, duration: 1.0)
        } else {
            showBanner("Copy failed", style: .error, duration: 1.0)
        }

        return didWrite
    }

    private func toggleHelpScreen() {
        guard !appUIState.showsThemeSettings else { return }
        guard !isCommandMode else {
            showBanner(
                AppConstants.Launcher.Help.commandModeInfoBanner,
                style: .info,
                duration: AppConstants.Launcher.Clipboard.infoBannerDuration
            )
            return
        }
        showsHelpScreen.toggle()
    }

    @discardableResult
    private func dismissHelpIfVisible() -> Bool {
        guard showsHelpScreen else { return false }
        showsHelpScreen = false
        return true
    }

    private func deleteClipboardResult(resultID: String) {
        guard let entryID = LauncherClipboardFeature.entryID(fromResultID: resultID) else { return }
        clipboardStore.deleteEntry(id: entryID)

        if selectedResultID == resultID {
            selectedResultID = displayedResults.first?.id
        }

        showBanner(
            AppConstants.Launcher.Clipboard.deletedBanner,
            style: .info,
            duration: AppConstants.Launcher.Clipboard.infoBannerDuration
        )
    }

    private func refreshClipboardSelectionIfNeeded() {
        guard !isCommandMode, isClipboardQuery else { return }

        if let selectedResultID,
           displayedResults.contains(where: { $0.id == selectedResultID }) {
            return
        }

        selectedResultID = displayedResults.first?.id
    }

    private func invalidateSearchRequests() {
        latestSearchID &+= 1
        searchTask?.cancel()
        searchTask = nil
    }

    private func beginSearchRequest() -> UInt64 {
        latestSearchID &+= 1
        return latestSearchID
    }

    private func refreshSearchResults() {
        guard !isCommandMode else { return }
        guard !isClipboardQuery else {
            invalidateSearchRequests()
            setInitialSelection()
            return
        }

        let currentQuery = query
        let searchLimit = AppConstants.Launcher.defaultSearchLimit
        let searchID = beginSearchRequest()
        searchTask?.cancel()
        searchTask = Task {
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            guard !Task.isCancelled else { return }

            let results = await Task.detached(priority: .userInitiated) {
                bridge.search(query: currentQuery, limit: searchLimit)
            }.value

            await MainActor.run {
                guard searchID == latestSearchID else { return }
                guard !isCommandMode, query == currentQuery else { return }
                backendResults = results
                setInitialSelection()
            }
        }
    }

    private func performWebSearchFromQuery() {
        guard !isCommandMode else { return }
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        if let translationCommand = extractTranslationQuery(from: trimmed) {
            handleTranslation(command: translationCommand)
            isQueryFocused = true
            return
        }

        var components = URLComponents(string: "https://www.google.com/search")
        components?.queryItems = [URLQueryItem(name: "q", value: trimmed)]
        guard let url = components?.url else { return }
        NSWorkspace.shared.open(url)
    }

    private func reloadConfig() {
        let result = themeStore.reloadFromConfig()
        let backendReloaded = bridge.reloadConfig()

        // Sync settings blur multiplier to AppUIState
        if let blurMultiplier = result.settingsBlurMultiplier {
            appUIState.settingsBlurMultiplier = blurMultiplier
        }

        var message = "Config reloaded"
        var style: BannerStyle = .info
        var duration: Double = 2.0
        var copyText: String? = nil

        if !backendReloaded {
            message = "Backend config reload failed"
            style = .error
            duration = 4.0
        } else if !result.warnings.isEmpty {
            message = result.warnings.joined(separator: ", ")
            style = .warning
            duration = 5.0
            copyText = result.warnings.joined(separator: "\n")
        }

        showBanner(message, style: style, copyText: copyText, duration: duration)
        if isCommandMode {
            commandFeedback = message
        }
        refreshSearchResults()
        focusActiveInput()
    }

    private func focusActiveInput(
        recoveryDelays: [Double] = [0.0, 0.04, 0.10],
        activateApp: Bool = true
    ) {
        if appUIState.showsThemeSettings {
            NotificationCenter.default.post(name: .lookFocusSettingsInputRequested, object: nil)
            return
        }

        focusRequestToken &+= 1
        let token = focusRequestToken

        if activateApp {
            NSApplication.shared.activate(ignoringOtherApps: true)
        }
        scheduleFocusRecovery(delays: recoveryDelays, token: token)
    }

    private func activateLauncherModeAndFocus() {
        if appUIState.showsThemeSettings {
            appUIState.showsThemeSettings = false
        }

        if isCommandMode {
            pendingKillCandidate = nil
            if activeCommandAcceptsInput {
                focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
            } else {
                isQueryFocused = false
            }
            return
        }

        focusActiveInput()
    }

    private func scheduleFocusRecovery(delays: [Double], token: UInt64) {
        for delay in delays {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                guard token == focusRequestToken else { return }
                guard !appUIState.showsThemeSettings else { return }
                guard let window = launcherWindow() else { return }

                if !window.isVisible {
                    window.makeKeyAndOrderFront(nil)
                } else {
                    window.makeKey()
                    window.orderFront(nil)
                }

                if let responder = findEditableTextField(in: window.contentView) {
                    window.makeFirstResponder(responder)
                }

                isQueryFocused = true
            }
        }
    }

    private func launcherWindow() -> NSWindow? {
        if let keyWindow = NSApplication.shared.keyWindow {
            return keyWindow
        }

        if let visibleWindow = NSApplication.shared.windows.first(where: { $0.isVisible }) {
            return visibleWindow
        }

        return NSApplication.shared.windows.first
    }

    private func findEditableTextField(in view: NSView?) -> NSView? {
        guard let view else { return nil }

        if let textField = view as? NSTextField,
            textField.isEditable,
            !textField.isHidden,
            textField.alphaValue > 0.01
        {
            return textField
        }

        for subview in view.subviews {
            if let found = findEditableTextField(in: subview) {
                return found
            }
        }

        return nil
    }

    private func toggleWindowVisibility() {
        if let window = launcherWindow(), window.isVisible && NSApplication.shared.isActive {
            hideLauncherWindow()
            return
        }

        captureFrontmostAppForRestoreIfNeeded()
        _ = bridge.requestIndexRefresh()
        NSApplication.shared.unhide(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)

        if let window = launcherWindow() {
            window.makeKeyAndOrderFront(nil)
            activateLauncherModeAndFocus()
            return
        }

        openWindow(id: "main")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            NSApplication.shared.activate(ignoringOtherApps: true)
            launcherWindow()?.makeKeyAndOrderFront(nil)
            activateLauncherModeAndFocus()
        }
    }

    private func hideLauncherWindow(restorePreviousApp: Bool = true) {
        guard let window = launcherWindow() else { return }
        focusRequestToken &+= 1
        isQueryFocused = false
        window.orderOut(nil)
        if restorePreviousApp {
            reactivatePreviouslyFocusedAppIfNeeded()
        } else {
            pidToRestoreOnHide = nil
        }
        refreshClipboardMonitoringMode()
    }

    private func bringOpenedAppToFront(appBundlePath: String) {
        let appURL = URL(fileURLWithPath: appBundlePath)
        guard let bundle = Bundle(url: appURL),
              let bundleID = bundle.bundleIdentifier
        else {
            return
        }

        let ownPID = ProcessInfo.processInfo.processIdentifier
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.postOpenActivationDelay) {
            // Skip if the user has since switched to a different app — don't steal focus back.
            if let frontmost = NSWorkspace.shared.frontmostApplication,
               frontmost.processIdentifier != ownPID,
               frontmost.bundleIdentifier != bundleID {
                return
            }
            let candidates = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID)
            if let app = candidates.first(where: { !$0.isTerminated }) ?? candidates.first {
                _ = app.activate()
            }
        }
    }

    private func captureFrontmostAppForRestoreIfNeeded() {
        guard let frontmost = NSWorkspace.shared.frontmostApplication else {
            pidToRestoreOnHide = nil
            return
        }

        if frontmost.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            pidToRestoreOnHide = nil
            return
        }

        pidToRestoreOnHide = frontmost.processIdentifier
    }

    private func reactivatePreviouslyFocusedAppIfNeeded() {
        guard let pid = pidToRestoreOnHide else { return }
        pidToRestoreOnHide = nil
        guard pid != ProcessInfo.processInfo.processIdentifier else { return }
        guard let app = NSRunningApplication(processIdentifier: pid) else { return }
        guard !app.isTerminated else { return }

        DispatchQueue.main.asyncAfter(deadline: .now() + Self.postHideActivationDelay) {
            _ = app.activate()
        }
    }

    private func refreshClipboardMonitoringMode() {
        let isVisible = launcherWindow()?.isVisible ?? false
        if NSApplication.shared.isActive && isVisible {
            clipboardStore.setMonitoringMode(.foreground)
        } else {
            clipboardStore.setMonitoringMode(.background)
        }
    }

    private func handleLookupTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after tw\" to translate", style: .error, duration: 3.2)
            return
        }

        Task {
            let results = await fetchAllTranslations(for: normalized)
            await MainActor.run {
                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Input",
                    sections: [
                        LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: results.en.dictionaryDefinition, failed: results.en.translated == nil),
                        LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: results.vi.dictionaryDefinition, failed: results.vi.translated == nil),
                        LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: results.ja.dictionaryDefinition, failed: results.ja.translated == nil),
                    ]
                )
            }
        }
    }

    private func previewLookupDefinition(for input: String) {
        lookupPreviewTask?.cancel()

        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard case .lookup(let text) = extractTranslationQuery(from: trimmed) else {
            lookupDefinition = nil
            return
        }

        let normalizedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedText.isEmpty else {
            lookupDefinition = nil
            return
        }

        let expectedQuery = trimmed
        lookupPreviewTask = Task {
            try? await Task.sleep(nanoseconds: 220_000_000)
            guard !Task.isCancelled else { return }

            let results = await fetchAllTranslations(for: normalizedText)

            guard !Task.isCancelled else { return }
            await MainActor.run {
                let latestQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
                guard latestQuery == expectedQuery else { return }
                lookupDefinition = LookupDefinition(
                    query: normalizedText,
                    sourceLabel: "Input",
                    sections: [
                        LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: results.en.dictionaryDefinition, failed: results.en.translated == nil),
                        LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: results.vi.dictionaryDefinition, failed: results.vi.translated == nil),
                        LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: results.ja.dictionaryDefinition, failed: results.ja.translated == nil),
                    ]
                )
            }
        }
    }

    private struct TranslationResult {
        let translated: String?
        let dictionaryDefinition: LookupPresentation?
    }

    private func fetchAllTranslations(for text: String) async -> (en: TranslationResult, vi: TranslationResult, ja: TranslationResult) {
        await withTaskGroup(of: (String, TranslationResult).self) { group in
            group.addTask {
                let translated = self.bridge.translate(text: text, targetLang: "en")?.translated
                let definition = await MainActor.run {
                    translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                }
                return ("en", TranslationResult(translated: translated, dictionaryDefinition: definition))
            }
            group.addTask {
                let translated = self.bridge.translate(text: text, targetLang: "vi")?.translated
                let definition = await MainActor.run {
                    translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                }
                return ("vi", TranslationResult(translated: translated, dictionaryDefinition: definition))
            }
            group.addTask {
                let translated = self.bridge.translate(text: text, targetLang: "ja")?.translated
                let definition = await MainActor.run {
                    translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                }
                return ("ja", TranslationResult(translated: translated, dictionaryDefinition: definition))
            }

            var en = TranslationResult(translated: nil, dictionaryDefinition: nil)
            var vi = TranslationResult(translated: nil, dictionaryDefinition: nil)
            var ja = TranslationResult(translated: nil, dictionaryDefinition: nil)
            for await (lang, result) in group {
                switch lang {
                case "en": en = result
                case "vi": vi = result
                case "ja": ja = result
                default: break
                }
            }
            return (en, vi, ja)
        }
    }

    private func fetchRawDefinition(for text: String) -> String? {
        let nsText = text as NSString
        let range = CFRange(location: 0, length: nsText.length)
        guard let unmanaged = DCSCopyTextDefinition(nil, text as CFString, range) else {
            return nil
        }
        let raw = (unmanaged.takeRetainedValue() as String)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return raw.isEmpty ? nil : raw
    }

    private func handleNetworkTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after t\" to translate", style: .error, duration: 3.2)
            return
        }

        lookupDefinition = LookupDefinition(
            query: normalized,
            sourceLabel: "Web",
            sections: [
                LookupTranslationSection(label: "Tiếng Việt", translated: nil, dictionaryDefinition: nil, failed: false),
                LookupTranslationSection(label: "English", translated: nil, dictionaryDefinition: nil, failed: false),
                LookupTranslationSection(label: "日本語", translated: nil, dictionaryDefinition: nil, failed: false),
            ]
        )

        Task {
            let results = await fetchNetworkTranslations(for: normalized)
            await MainActor.run {
                let hasAnyResult = results.en.translated != nil
                    || results.vi.translated != nil
                    || results.ja.translated != nil

                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Web",
                    sections: [
                        LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: nil, failed: results.vi.translated == nil),
                        LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: nil, failed: results.en.translated == nil),
                        LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: nil, failed: results.ja.translated == nil),
                    ]
                )

                if !hasAnyResult {
                    let message = results.en.errorMessage
                        ?? results.vi.errorMessage
                        ?? results.ja.errorMessage
                        ?? "Translation failed"
                    showBanner(message, style: .error, duration: 3.2)
                }
            }
        }
    }

    private struct NetworkTranslationResult {
        let translated: String?
        let errorMessage: String?
    }

    private func fetchNetworkTranslations(for text: String) async -> (en: NetworkTranslationResult, vi: NetworkTranslationResult, ja: NetworkTranslationResult) {
        await withTaskGroup(of: (String, NetworkTranslationResult).self) { group in
            group.addTask {
                let result = self.bridge.translate(text: text, targetLang: "en")
                let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                return (
                    "en",
                    NetworkTranslationResult(
                        translated: (translated?.isEmpty == false) ? translated : nil,
                        errorMessage: result?.error?.userFacingMessage
                    )
                )
            }
            group.addTask {
                let result = self.bridge.translate(text: text, targetLang: "vi")
                let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                return (
                    "vi",
                    NetworkTranslationResult(
                        translated: (translated?.isEmpty == false) ? translated : nil,
                        errorMessage: result?.error?.userFacingMessage
                    )
                )
            }
            group.addTask {
                let result = self.bridge.translate(text: text, targetLang: "ja")
                let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                return (
                    "ja",
                    NetworkTranslationResult(
                        translated: (translated?.isEmpty == false) ? translated : nil,
                        errorMessage: result?.error?.userFacingMessage
                    )
                )
            }

            var en = NetworkTranslationResult(translated: nil, errorMessage: nil)
            var vi = NetworkTranslationResult(translated: nil, errorMessage: nil)
            var ja = NetworkTranslationResult(translated: nil, errorMessage: nil)
            for await (lang, result) in group {
                switch lang {
                case "en": en = result
                case "vi": vi = result
                case "ja": ja = result
                default: break
                }
            }
            return (en, vi, ja)
        }
    }

    private func showBanner(
        _ message: String,
        style: BannerStyle = .info,
        copyText: String? = nil,
        duration: Double = 1.8
    ) {
        bannerTask?.cancel()
        bannerStyle = style
        bannerCopyText = copyText
        withAnimation(.easeOut(duration: 0.15)) {
            bannerMessage = message
        }

        bannerTask = Task {
            let ns = UInt64(max(0.6, duration) * 1_000_000_000)
            try? await Task.sleep(nanoseconds: ns)
            guard !Task.isCancelled else { return }
            await MainActor.run {
                withAnimation(.easeIn(duration: 0.15)) {
                    bannerMessage = nil
                    bannerCopyText = nil
                }
            }
        }
    }

    private func selectCommand(_ commandID: String) {
        pendingKillCandidate = nil
        selectedKillSuggestionIndex = nil
        if commandID != AppConstants.Launcher.Command.kill {
            recentlyKilledPIDs.removeAll()
        }
        activeCommandID = commandID
        selectedCommandID = commandID
        commandInput = ""
        commandFeedback = "Selected /\(commandID)"
        requestCommandInputFocusIfNeeded()
    }

    @ViewBuilder
    private var commandModeView: some View {
        GeometryReader { proxy in
            let splitSpacing: CGFloat = 8
            let dividerWidth: CGFloat = 1
            let usableWidth = max(0, proxy.size.width - splitSpacing - dividerWidth)
            let leftWidth = max(170, usableWidth * 0.25)

            HStack(spacing: splitSpacing) {
                CommandListView(
                    commands: commandCatalog,
                    selectedID: selectedCommandID,
                    activeID: activeCommandID,
                    themeStore: themeStore,
                    onSelect: selectCommand
                )
                .frame(width: leftWidth)
                .frame(maxHeight: .infinity, alignment: .topLeading)

                Rectangle()
                    .fill(themeStore.dividerColor())
                    .frame(width: dividerWidth)
                    .padding(.vertical, 2)

                VStack(alignment: .leading, spacing: 6) {
                    if let activeCommand {
                        if activeCommandAcceptsInput {
                            CommandInputBar(
                                text: $commandInput,
                                command: activeCommand,
                                isQueryFocused: $isQueryFocused,
                                themeStore: themeStore,
                                onSubmit: handleSubmit
                            )
                        } else {
                            CommandHeaderBar(
                                command: activeCommand,
                                themeStore: themeStore,
                                subtitle: "Read-only command"
                            )
                        }
                    }

                    ZStack(alignment: .topLeading) {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(themeStore.panelFillColor())

                    if activeCommandID == AppConstants.Launcher.Command.kill {
                        let killSearchTerm = commandArgsPart.trimmingCharacters(in: .whitespacesAndNewlines)
                        let portQuery = killSearchTerm.hasPrefix(":") || killSearchTerm.lowercased().hasPrefix("port ")
                        let defaultKillEmptyMessage = portQuery
                            ? "No process listening on this port"
                            : "No matches. Type an app name or use :3000"

                        KillCommandView(
                            suggestions: Array(killSuggestions),
                            selectedIndex: selectedKillSuggestionIndex,
                            emptyMessage: commandFeedback.isEmpty ? defaultKillEmptyMessage : commandFeedback,
                            themeStore: themeStore,
                            onSelect: { candidate in
                                pendingKillCandidate = candidate
                                selectedKillSuggestionIndex = candidate.number
                            }
                        )
                            .onAppear {
                                if selectedKillSuggestionIndex == nil {
                                    selectedKillSuggestionIndex = killSuggestions.first?.number
                                }
                            }
                            .padding(8)
                        } else if activeCommandID == AppConstants.Launcher.Command.sys {
                            SystemInfoView(items: SystemInfoCommand.getSystemInfoItems(), themeStore: themeStore)
                                .padding(8)
                        } else {
                            VStack(alignment: .leading, spacing: 0) {
                                CommandFeedbackView(
                                    message: liveCommandPreview ?? (commandFeedback.isEmpty ? AppConstants.Launcher.commandEmptyMessage : commandFeedback),
                                    themeStore: themeStore
                                )
                                Spacer(minLength: 0)
                            }
                            .padding(10)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }

    var body: some View {
        let windowCornerRadius = AppConstants.Launcher.windowCornerRadius
        let contentSpacing: CGFloat = isCommandMode ? 8 : 12
        let contentPadding: CGFloat = isCommandMode ? 10 : 14

        ZStack {
            themedBackground

            VStack(alignment: .leading, spacing: contentSpacing) {
                if appUIState.showsThemeSettings {
                    ThemeSettingsView(settings: $themeStore.settings)
                } else {
                    if !isCommandMode {
                        SearchInputBar(
                            text: $query,
                            isCommandMode: $isCommandMode,
                            isQueryFocused: $isQueryFocused,
                            activeCommand: activeCommand,
                            themeStore: themeStore,
                            onSubmit: handleSubmit,
                            onExitCommandMode: exitCommandMode
                        )
                    }

                    if let bannerMessage {
                        HStack(spacing: 8) {
                            Text(bannerMessage)
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                                .foregroundStyle(themeStore.fontColor())
                            if let copyText = bannerCopyText {
                                Button("Copy") {
                                    NSPasteboard.general.clearContents()
                                    NSPasteboard.general.setString(copyText, forType: .string)
                                    showBanner("Copied", style: .info, duration: 1.0)
                                }
                                .buttonStyle(.plain)
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 3)
                                .background(.white.opacity(0.18), in: Capsule())
                            }
                        }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(bannerStyle.background, in: Capsule())
                            .transition(.move(edge: .top).combined(with: .opacity))
                    }

                    if isCommandMode {
                        commandModeView
                    } else if isTranslationQuery {
                        LookupDefinitionPanelView(
                            definition: lookupDefinition,
                            emptyHint: translationEmptyHint,
                            isWebMode: isWebTranslationQuery,
                            themeStore: themeStore
                        )
                    } else {
                        if showsHelpScreen {
                            LauncherHelpScreenView(themeStore: themeStore)
                        } else if isClipboardQuery && displayedResults.isEmpty {
                            ClipboardEmptyStateView(themeStore: themeStore)
                        } else {
                            HStack(spacing: 0) {
                                ResultsListView(
                                    results: displayedResults,
                                    selectedID: selectedResultID,
                                    themeStore: themeStore,
                                    onSelect: { selectedResultID = $0 },
                                    onOpen: { _ in openSelectedApp() }
                                )

                                if let selectedID = selectedResultID,
                                   let selectedResult = displayedResults.first(where: { $0.id == selectedID }) {
                                    Rectangle()
                                        .fill(.white.opacity(0.08))
                                        .frame(width: 1)
                                        .padding(.vertical, 4)

                                    ResultPreviewView(
                                        result: selectedResult,
                                        onDeleteClipboard: selectedResult.kind == .clipboard
                                            ? { deleteClipboardResult(resultID: selectedResult.id) }
                                            : nil
                                    )
                                }
                            }
                        }
                    }

                    if isCommandMode {
                        Spacer(minLength: 0)
                    }

                    if !isKillConfirmationVisible {
                        HintBar(hint: currentHint, themeStore: themeStore)
                    }
                }
            }
            .padding(contentPadding)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .font(themeStore.uiFont())
            .foregroundStyle(themeStore.fontColor())
            .background(.black.opacity(0.16), in: RoundedRectangle(cornerRadius: windowCornerRadius, style: .continuous))
            .contentShape(Rectangle())
            .onTapGesture {
                focusActiveInput()
            }
        }
        .background(Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: windowCornerRadius, style: .continuous))
        .overlay {
            let borderWidth = themeStore.borderLineWidth()
            if borderWidth > 0 {
                RoundedRectangle(cornerRadius: windowCornerRadius, style: .continuous)
                    .strokeBorder(
                        hasSudoWarning ? Color.orange.opacity(0.95) : themeStore.borderColor(),
                        lineWidth: borderWidth
                    )
            }
        }
        .overlay(alignment: .topTrailing) {
            if shouldShowTestHint {
                Text("TEST APP")
                    .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .bold))
                    .foregroundStyle(Color.red.opacity(0.95))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(.black.opacity(0.35), in: Capsule())
                    .padding(.top, 8)
                    .padding(.trailing, 10)
            }
        }
        .overlay(alignment: .bottomTrailing) {
            Link("© 2026 by Kunkka", destination: URL(string: "https://github.com/kunkka19xx")!)
                .font(themeStore.uiFont(size: CGFloat(max(9, themeStore.settings.fontSize - 4)), weight: .regular))
                .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.50))
                .padding(.trailing, 10)
                .padding(.bottom, 8)
        }
        .overlay(alignment: .bottom) {
            if isCommandMode,
               activeCommandID == AppConstants.Launcher.Command.kill,
               let pendingKillCandidate
            {
                KillConfirmationBar(
                    candidate: pendingKillCandidate,
                    themeStore: themeStore,
                    onConfirm: {
                        runKillCommand(candidate: pendingKillCandidate)
                        self.pendingKillCandidate = nil
                    },
                    onCancel: {
                        self.pendingKillCandidate = nil
                    }
                )
                .padding(.horizontal, 14)
                .padding(.bottom, 24)
            }
        }
        .ignoresSafeArea()
        .onAppear {
            refreshSearchResults()
            startKeyboardNavigationIfNeeded()
            focusActiveInput()
            refreshClipboardMonitoringMode()
        }
        .onDisappear {
            invalidateSearchRequests()
            bannerTask?.cancel()
            lookupPreviewTask?.cancel()
            keyboardMonitor.stop()
            clipboardStore.setMonitoringMode(.background)
        }
        .onChange(of: query) { _, _ in
            previewLookupDefinition(for: query)
            if !isCommandMode {
                if showsHelpScreen {
                    showsHelpScreen = false
                }
                if isClipboardQuery {
                    setInitialSelection()
                } else {
                    refreshSearchResults()
                }
            }
        }
        .onReceive(clipboardStore.$entries) { _ in
            refreshClipboardSelectionIfNeeded()
        }
        .onChange(of: commandInput) { _, _ in
            if isCommandMode {
                if commandArgsPart.isEmpty, activeCommandID != AppConstants.Launcher.Command.sys {
                    commandFeedback = ""
                }
                if activeCommandID == AppConstants.Launcher.Command.kill {
                    if selectedKillSuggestionIndex != nil || pendingKillCandidate != nil {
                        logUIEvent("kill input changed -> clear pending/select input='\(commandArgsPart)'")
                    }
                    pendingKillCandidate = nil
                    selectedKillSuggestionIndex = nil
                }
                setInitialSelection()
            }
        }
        .onChange(of: appUIState.showsThemeSettings) { _, showsSettings in
            if showsSettings {
                showsHelpScreen = false
                keyboardMonitor.stop()
                NotificationCenter.default.post(name: .lookFocusSettingsInputRequested, object: nil)
            } else {
                startKeyboardNavigationIfNeeded()
                focusActiveInput()
            }
        }
        .onMoveCommand { direction in
            moveSelection(direction)
        }
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
        ) { _ in
            focusActiveInput()
            refreshClipboardMonitoringMode()
        }
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didResignActiveNotification)
        ) { _ in
            refreshClipboardMonitoringMode()
        }
        .onReceive(NotificationCenter.default.publisher(for: .lookReloadConfigRequested)) { _ in
            reloadConfig()
        }
        .onReceive(NotificationCenter.default.publisher(for: .lookRefocusInputRequested)) { _ in
            DispatchQueue.main.async {
                focusActiveInput(recoveryDelays: [0.0], activateApp: false)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .lookActivateLauncherRequested)) { _ in
            activateLauncherModeAndFocus()
            refreshClipboardMonitoringMode()
        }
        .onReceive(NotificationCenter.default.publisher(for: .lookHideLauncherRequested)) { _ in
            hideLauncherWindow()
        }
        .onReceive(NotificationCenter.default.publisher(for: .lookToggleWindowRequested)) { _ in
            toggleWindowVisibility()
            refreshClipboardMonitoringMode()
        }
    }

    @ViewBuilder
    private var themedBackground: some View {
        if let image = themeStore.backgroundImage {
            backgroundImageView(image: image)
                .blur(radius: themeStore.settings.backgroundImageBlur)
                .opacity(themeStore.settings.backgroundImageOpacity)
        }

        VisualEffectBlur(material: themeStore.settings.blurMaterial.material)
            .opacity(
                min(
                    1,
                    max(
                        0,
                        themeStore.settings.blurOpacity
                            * themeStore.settings.blurMaterial.blurOpacityScale
                            * (appUIState.showsThemeSettings ? appUIState.settingsBlurMultiplier : 1.0)
                    )
                )
            )

        Color(
            .sRGB,
            red: themeStore.settings.tintRed,
            green: themeStore.settings.tintGreen,
            blue: themeStore.settings.tintBlue,
            opacity: min(
                1,
                max(
                    0,
                    themeStore.settings.tintOpacity * themeStore.settings.blurMaterial.tintOpacityScale
                )
            )
        )
    }

    @ViewBuilder
    private func backgroundImageView(image: NSImage) -> some View {
        GeometryReader { proxy in
            let size = proxy.size

            Group {
                switch themeStore.settings.backgroundImageMode {
                case .fit:
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFit()
                        .frame(width: size.width, height: size.height)
                case .fill:
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFill()
                        .frame(width: size.width, height: size.height)
                        .clipped()
                case .stretch:
                    Image(nsImage: image)
                        .resizable()
                        .frame(width: size.width, height: size.height)
                case .tile:
                    Rectangle()
                        .fill(ImagePaint(image: Image(nsImage: image), scale: 0.3))
                        .frame(width: size.width, height: size.height)
                }
            }
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
    }

    private func startKeyboardNavigationIfNeeded() {
        guard !appUIState.showsThemeSettings else { return }
        keyboardMonitor.start(
            onNext: {
                moveSelection(.down, shouldAutocompleteCommand: true, preferCommandListInCommandMode: true)
            },
            onPrevious: {
                moveSelection(.up, shouldAutocompleteCommand: true, preferCommandListInCommandMode: true)
            },
            onArrowDown: {
                if isCommandMode {
                    if activeCommandID == AppConstants.Launcher.Command.kill {
                        moveSelection(.down)
                    }
                } else {
                    moveSelection(.down)
                }
            },
            onArrowUp: {
                if isCommandMode {
                    if activeCommandID == AppConstants.Launcher.Command.kill {
                        moveSelection(.up)
                    }
                } else {
                    moveSelection(.up)
                }
            },
            onEnterCommandMode: {
                if !isCommandMode {
                    enterCommandMode()
                }
            },
            onExitCommandMode: {
                exitCommandMode()
            },
            onHideLauncher: {
                hideLauncherWindow()
            },
            inCommandMode: { isCommandMode },
            onWebSearch: {
                performWebSearchFromQuery()
            },
            onRevealInFinder: {
                revealSelectedInFinder()
            },
            onCopySelection: {
                copySelectedResultToPasteboard()
            },
            onToggleHelp: {
                toggleHelpScreen()
            },
            onDismissHelpIfVisible: {
                dismissHelpIfVisible()
            },
            onSelectCommandByIndex: { [self] index in
                guard index > 0 && index <= commandCatalog.count else { return }
                let command = commandCatalog[index - 1]
                pendingKillCandidate = nil
                selectedKillSuggestionIndex = nil
                activeCommandID = command.id
                selectedCommandID = command.id
                commandFeedback = "Selected /\(command.id)"
                requestCommandInputFocusIfNeeded()
            },
            onConfirmKill: { [self] in
                if let pendingKillCandidate {
                    runKillCommand(candidate: pendingKillCandidate)
                    self.pendingKillCandidate = nil
                }
            },
            onCancelKill: { [self] in
                pendingKillCandidate = nil
            },
            killConfirmationActive: { [self] in
                pendingKillCandidate != nil
            }
        )
    }
}
