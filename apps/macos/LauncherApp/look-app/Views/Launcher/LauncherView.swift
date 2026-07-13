import AppKit
import CoreServices
import OSLog
import SwiftUI

struct LauncherView: View {

    enum TranslationCommand {
        case network(String)
        case lookup(String)
    }

    enum BannerStyle {
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

    @EnvironmentObject var appUIState: AppUIState
    @EnvironmentObject var themeStore: ThemeStore
    @Environment(\.openWindow) var openWindow
    @StateObject var clipboardStore = ClipboardHistoryStore()
    @StateObject var aiAnswer = AIAnswerController()

    @State var query = ""
    @State var commandInput = ""
    @State var isCommandMode = false
    @State var backendResults: [LauncherResult] = []
    @State var webSuggestions: [String] = []
    @State var webSuggestionTask: Task<Void, Never>?
    @State var recentURLEntries: [URLHistoryEntry] = []
    @State var recentURLTask: Task<Void, Never>?
    // Quick Actions for the selected result (see docs/writing-controls.md).
    @State var quickActionDescriptors: [QuickActionDescriptor] = []
    @State var quickActionStates: [String: ActionState] = [:]
    @State var quickActionTask: Task<Void, Never>?
    @State var selectedResultID: String?
    @State var pickedKeys: [String] = []
    @State var pickedResultsByKey: [String: LauncherResult] = [:]

    static func pickedKey(for result: LauncherResult) -> String {
        "\(result.kind.rawValue)|\(result.path)"
    }
    @State var selectedCommandID: String?
    @State var activeCommandID: String?
    @State var commandFeedback = ""
    @State var keyboardMonitor = KeyboardSelectionMonitor()
    @State var searchTask: Task<Void, Never>?
    @State var latestSearchID: UInt64 = 0
    @State var bannerMessage: String?
    @State var bannerStyle: BannerStyle = .info
    @State var bannerCopyText: String?
    @State var bannerTask: Task<Void, Never>?
    @State var lookupPreviewTask: Task<Void, Never>?
    @State var selectedKillSuggestionIndex: Int?
    @State var pendingKillCandidate: KillCommand.Candidate?
    // nil == no empty-Trash confirmation pending; otherwise the item count to show.
    // (Moving files/folders to Trash is recoverable, so it skips confirmation;
    // only the permanent Empty Trash prompts.)
    @State var pendingEmptyTrashCount: Int?
    // True while a trash/empty operation is running, to block re-triggering it.
    @State var isDeleteInFlight = false
    @State var killListRefreshTick: Int = 0
    @State var recentlyKilledPIDs: Set<Int32> = []
    @State var showsHelpScreen = false
    @State var focusRequestToken: UInt64 = 0
    @State var lookupDefinition: LookupDefinition?
    @State var pidToRestoreOnHide: pid_t?
    @StateObject var runningAppsService = RunningAppsService()

    /// Live size of the whole panel, captured so a background image can be
    /// cropped into each floating tile at its correct window position (the tiles
    /// share one aligned image, cut apart by the gaps). See `tileBackground`.
    @State private var panelSize: CGSize = .zero
    static let panelCoordinateSpace = "launcherPanel"

    var runningAppsPlacement: RunningAppsPlacement {
        themeStore.settings.runningAppsPlacement
    }

    var shouldShowRunningAppsStrip: Bool {
        runningAppsPlacement != .none
            && !isCommandMode
            && !appUIState.showsThemeSettings
            && !showsHelpScreen
            && !runningAppsService.items.isEmpty
    }

    /// Activates the strip icon assigned to Cmd+`key`. The key is mapped
    /// to a visual position via the ergonomic layout in
    /// `AppConstants.Launcher.RunningAppsStrip.visualPosition(forKey:total:)`.
    /// On success the launcher is *not* hidden here - instead we let
    /// `didResignActiveNotification` close it, which only fires after
    /// macOS has handed key-window status to the target app. Hiding
    /// synchronously raced that handoff and left the keyboard focused
    /// nowhere visible.
    @discardableResult
    func activateRunningApp(forKey key: Int) -> Bool {
        let log = RunningAppsLog.logger
        let total = runningAppsService.items.count

        if runningAppsPlacement == .none || isCommandMode || appUIState.showsThemeSettings {
            log.debug("⌘+\(key, privacy: .public) declined (placement=\(self.runningAppsPlacement.rawValue, privacy: .public) cmd=\(self.isCommandMode, privacy: .public) settings=\(self.appUIState.showsThemeSettings, privacy: .public))")
            return false
        }
        guard let position = AppConstants.Launcher.RunningAppsStrip.visualPosition(forKey: key, total: total) else {
            log.debug("⌘+\(key, privacy: .public) declined (no slot for this key, items=\(total, privacy: .public))")
            return false
        }

        let target = runningAppsService.items[position]
        let activated = runningAppsService.activate(index: position)
        log.debug("⌘+\(key, privacy: .public) -> position \(position, privacy: .public) \(target.name, privacy: .public) (activated=\(activated, privacy: .public))")
        // If activate failed (process gone, denied, etc.) didResignActive
        // won't fire and the launcher stays visible for another attempt.
        return activated
    }

    static let postHideActivationDelay: TimeInterval = 0.01
    @FocusState var isQueryFocused: Bool

    let bridge = EngineBridge.shared
    let shouldShowTestHint = LauncherView.cachedShouldShowTestHint

    static let cachedShouldShowTestHint: Bool = {
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

    static let debugEventLoggingEnabled: Bool = {
        let env = ProcessInfo.processInfo.environment
        let raw = env["LOOK_UI_DEBUG_EVENTS"] ?? env["LOOK_DEV_HINT"] ?? ""
        return ["1", "true", "yes", "on"].contains(raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    }()

    static let logger = Logger(subsystem: "noah-code.Look", category: "ui")

    func logUIEvent(_ message: String) {
        guard Self.debugEventLoggingEnabled else { return }
        Self.logger.notice("\(message, privacy: .public)")
    }

    static let clipboardSubtitleDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()

    let commandCatalog: [AppCommand] = AppConstants.Launcher.commandCatalog

    var pinnedLookupScope: LauncherPinnedLookupScope {
        LauncherSearchLogic.pinnedLookupScope(for: query)
    }

    var normalizedPinnedLookupQuery: String? {
        LauncherSearchLogic.normalizedPinnedLookupQuery(for: query, scope: pinnedLookupScope)
    }

    var shouldInjectFinderResult: Bool {
        LauncherSearchLogic.shouldInjectFinder(
            normalizedQuery: normalizedPinnedLookupQuery,
            scope: pinnedLookupScope
        )
    }

    var quickFolderPinnedResults: [LauncherResult] {
        guard pinnedLookupScope == .unscoped || pinnedLookupScope == .folders else { return [] }
        guard let normalized = normalizedPinnedLookupQuery else { return [] }

        return AppConstants.Launcher.QuickFolder.entries.compactMap { entry in
            let normalizedTitle = entry.title.lowercased()
            let isMatch = normalizedTitle.contains(normalized)
                || (normalizedTitle.hasPrefix(normalized)
                    && normalized.count >= AppConstants.Launcher.QuickFolder.minPrefixMatchLength)
            guard isMatch else { return nil }

            let folderPath = entry.resolvedPath(homeDirectory: NSHomeDirectory())
            guard FileManager.default.fileExists(atPath: folderPath) else { return nil }

            return LauncherResult(
                id: "\(AppConstants.Launcher.QuickFolder.idPrefix)\(normalizedTitle)",
                kind: .folder,
                title: entry.title,
                subtitle: entry.subtitle ?? AppConstants.Launcher.QuickFolder.pinnedSubtitle,
                path: folderPath,
                score: AppConstants.Launcher.Finder.pinnedScore
            )
        }
    }

    var finderPinnedResult: LauncherResult {
        LauncherResult(
            id: AppConstants.Launcher.Finder.pinnedResultID,
            kind: .app,
            title: "Finder",
            subtitle: AppConstants.Launcher.Finder.pinnedSubtitle,
            path: AppConstants.Launcher.Finder.appPath,
            score: AppConstants.Launcher.Finder.pinnedScore
        )
    }

    var backendFilteredResults: [LauncherResult] {
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

    var isClipboardQuery: Bool {
        LauncherClipboardFeature.isClipboardQuery(query)
    }

    var isRecentQuery: Bool {
        query.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .hasPrefix(AppConstants.Launcher.QueryPrefix.recent)
    }

    /// A leading `"` opens the prefix-discovery menu: a list of every query
    /// prefix with a short description. Typing after the `"` (e.g. `"folder`)
    /// filters the list by name/description; picking one fills the prefix in.
    var isPrefixSuggestionQuery: Bool {
        query.trimmingCharacters(in: .whitespacesAndNewlines)
            .hasPrefix(AppConstants.Launcher.QueryPrefix.discovery)
    }

    /// The text typed after the leading `"`, used to filter the discovery menu.
    var prefixSuggestionFilter: String {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let discovery = AppConstants.Launcher.QueryPrefix.discovery
        guard trimmed.hasPrefix(discovery) else { return "" }
        return String(trimmed.dropFirst(discovery.count))
    }

    /// Synthetic results backing the discovery menu. Rendered through the normal
    /// results list so selection/keyboard nav work as usual; `openSelectedApp`
    /// recognises the id prefix and inserts the prefix instead of opening.
    var prefixSuggestionResults: [LauncherResult] {
        let suggestions = AppConstants.Launcher.PrefixSuggestion.menuEntries(matching: prefixSuggestionFilter)
        return suggestions.enumerated().map { index, entry in
            LauncherResult(
                id: "\(AppConstants.Launcher.PrefixSuggestion.resultIDPrefix)\(entry.prefix)",
                kind: .app,
                title: entry.displayWithArg,
                subtitle: entry.description,
                path: "",
                // Preserve list order: higher score sorts first, top entry highest.
                score: suggestions.count - index
            )
        }
    }

    /// A leading `:` opens the command-discovery menu: every command with its
    /// description. Typing after the `:` (e.g. `:process`) filters by id and
    /// description; picking one enters that command. A `:<exact-id> <args>`
    /// live-trigger (e.g. `:calc 2+2`) jumps straight into the command instead
    /// (handled in `onChange(of: query)`), so it isn't a discovery query.
    var isCommandSuggestionQuery: Bool {
        guard !isCommandMode else { return false }
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(":") else { return false }
        if let cmd = extractInlineCommand(from: query), cmd.hasSpace { return false }
        return true
    }

    /// The text typed after the leading `:`, used to filter the command menu.
    var commandSuggestionFilter: String {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(":") else { return "" }
        return String(trimmed.dropFirst())
    }

    /// Synthetic results backing the command-discovery menu. Rendered through the
    /// normal results list; `openSelectedApp` recognises the id prefix and enters
    /// the command instead of opening a file.
    var commandSuggestionResults: [LauncherResult] {
        let matches = AppConstants.Launcher.commandCatalog(matching: commandSuggestionFilter)
        return matches.enumerated().map { index, command in
            LauncherResult(
                id: "\(AppConstants.Launcher.CommandSuggestion.resultIDPrefix)\(command.id)",
                kind: .app,
                title: command.title,
                subtitle: command.detail,
                path: "",
                score: matches.count - index
            )
        }
    }

    var clipboardSearchTerm: String? {
        LauncherClipboardFeature.searchTerm(from: query)
    }

    var clipboardResults: [LauncherResult] {
        guard let clipboardSearchTerm else { return [] }

        return clipboardStore.search(clipboardSearchTerm).map { entry in
            LauncherClipboardFeature.makeResult(entry: entry, dateFormatter: Self.clipboardSubtitleDateFormatter)
        }
    }

    /// Google autocomplete rows, appended after the engine results. Built by
    /// hand like `prefixSuggestionResults`; `openSelectedApp` recognises the id
    /// prefix and runs a web search instead of opening a file.
    var webSuggestionResults: [LauncherResult] {
        guard allowsSuggestionRows else { return [] }
        return webSuggestions.enumerated().map { index, text in
            LauncherResult(
                id: "\(AppConstants.Launcher.WebSuggestion.resultIDPrefix)\(text)",
                kind: .app,
                title: text,
                subtitle: "Search Google",
                path: "",
                score: -1 - index
            )
        }
    }

    // URL-aware rows (urlResult, recentURLResults, mergeByScore) live in
    // LauncherView+URLResults.swift.

    var displayedResults: [LauncherResult] {
        if isPrefixSuggestionQuery { return prefixSuggestionResults }
        if isCommandSuggestionQuery { return commandSuggestionResults }
        if isClipboardQuery { return clipboardResults }
        // Recent URLs interleave with local results by frecency; web-search rows
        // stay last.
        let ranked = mergeByScore(backendFilteredResults, recentURLResults)
        let tail = webSuggestionResults
        guard let urlResult else {
            return ranked + tail
        }
        // Structural matches can't be a file/search, so rank on top. A bare-host
        // match must never take the default slot from a real local result, so it
        // sits after the backend results (issue #232).
        switch urlResult.tier {
        case .structural:
            return [urlResult.result] + ranked + tail
        case .bareHost:
            return ranked + [urlResult.result] + tail
        }
    }

    var isTranslationQuery: Bool {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        return extractTranslationQuery(from: trimmed) != nil
    }

    var translationEmptyHint: String? {
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

    var isWebTranslationQuery: Bool {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let command = extractTranslationQuery(from: trimmed) else {
            return false
        }

        if case .network = command {
            return true
        }
        return false
    }

    var currentHint: String {
        hintItems.joined(separator: "  •  ")
    }

    var hintItems: [String] {
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
                return ["Esc back", "Tab/Cmd+1-6 switch", "Cmd+/ command mode", "Cmd+Shift+, settings"]
            }
            if activeCommandID == AppConstants.Launcher.Command.pomo {
                return ["Space start/pause", "R reset", "P music", "Esc back", "Tab/Cmd+1-6 switch"]
            }
            if activeCommandID == AppConstants.Launcher.Command.todo {
                return ["Cmd+N switch page", "Cmd+S save", "Cmd+1-6 switch", "Esc back"]
            }
            return ["Enter run", "Tab select", "Cmd+1-6 switch", "Esc back"]
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

        if isPrefixSuggestionQuery {
            return ["Enter pick prefix", "Up/Down move", "Esc clear", "Cmd+H help"]
        }

        if isCommandSuggestionQuery {
            return ["Enter run command", "Up/Down move", "Esc clear", "Cmd+H help"]
        }

        if isClipboardQuery {
            return ["Enter copy clip", "Delete remove clip"]
        }

        // The home screen replaces the "Cmd+/ command mode" hint with a
        // clickable today done/total quick view (see todoQuickView), so it
        // is intentionally omitted here.
        return ["Enter open", "Cmd+H help"]
    }

    /// True when the launcher is on its default/home screen (the state
    /// whose hint falls through to the list above), where the /todo quick
    /// view is shown in place of the command-mode hint.
    var isHomeHintScreen: Bool {
        guard !appUIState.showsThemeSettings, !isCommandMode, !showsHelpScreen else { return false }
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        if extractTranslationQuery(from: trimmed) != nil { return false }
        if isPrefixSuggestionQuery || isCommandSuggestionQuery || isClipboardQuery { return false }
        return true
    }

    /// Today's done/total quick view for the home hint bar, or nil when
    /// off the home screen or today has no tasks (then it stays empty).
    var todoQuickView: HintBar.TodoQuickView? {
        guard isHomeHintScreen else { return nil }
        let state = TodoSharedState.shared
        let stat = state.todayStat
        guard stat.total > 0 else { return nil }
        let open = state.today?.tasks.filter { !$0.done }.map(\.name) ?? []
        return HintBar.TodoQuickView(done: stat.done, total: stat.total, openTasks: open) {
            enterCommandMode(commandID: AppConstants.Launcher.Command.todo, prefilledInput: "")
        }
    }

    var commandNamePart: String {
        guard activeCommandID == nil else { return "" }
        let normalized = commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return "" }
        return normalized.split(maxSplits: 1, whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? ""
    }

    var commandArgsPart: String {
        if activeCommandID != nil {
            return commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
        }

        let normalized = commandInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let splitPoint = normalized.firstIndex(where: { $0.isWhitespace }) else { return "" }
        return String(normalized[splitPoint...]).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var activeCommand: AppCommand? {
        guard let activeCommandID else { return nil }
        return commandCatalog.first(where: { $0.id == activeCommandID })
    }

    var activeCommandAcceptsInput: Bool {
        guard let activeCommandID else { return false }
        if activeCommandID == AppConstants.Launcher.Command.sys { return false }
        if activeCommandID == AppConstants.Launcher.Command.pomo { return false }
        // /todo owns its own top search bar, like /pomo owns its header.
        if activeCommandID == AppConstants.Launcher.Command.todo { return false }
        return true
    }

    var isKillConfirmationVisible: Bool {
        isCommandMode
            && activeCommandID == AppConstants.Launcher.Command.kill
            && pendingKillCandidate != nil
    }

    var isDeleteConfirmationVisible: Bool {
        !isCommandMode && pendingEmptyTrashCount != nil
    }

    var liveCommandPreview: String? {
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

    var hasSudoWarning: Bool {
        guard isCommandMode, activeCommandID == AppConstants.Launcher.Command.shell else { return false }
        return ShellCommand.hasSudoWarning(commandInput)
    }

    var filteredCommands: [AppCommand] {
        let prefix = commandNamePart.lowercased()
        if prefix.isEmpty {
            return commandCatalog
        }
        return commandCatalog.filter { $0.id.hasPrefix(prefix) }
    }

    var killSuggestions: [KillCommand.Candidate] {
        _ = killListRefreshTick
        let searchTerm = commandArgsPart.trimmingCharacters(in: .whitespacesAndNewlines)
        return KillCommand.suggestions(searchTerm: searchTerm)
            .filter { !recentlyKilledPIDs.contains($0.pid) }
    }

    var body: some View {
        let windowCornerRadius = AppConstants.Launcher.windowCornerRadius
        // When floating, use a single uniform gap between the top row and the
        // columns so it matches the horizontal gap between the columns (i3 style);
        // otherwise keep the classic fixed spacing.
        let contentSpacing: CGFloat = showsFloatingCards ? innerGap : (isCommandMode ? 8 : 12)
        let contentPadding: CGFloat = isCommandMode ? 10 : 14

        // Running apps render inside the search bar (see panelContent), not as a
        // floating strip that grows the window. The launcher is always a single
        // fixed-size panel regardless of the running-apps toggle.
        borderedPanel(windowCornerRadius: windowCornerRadius, contentSpacing: contentSpacing, contentPadding: contentPadding)
        .ignoresSafeArea()
        .onAppear {
            refreshSearchResults()
            startKeyboardNavigationIfNeeded()
            focusActiveInput()
            refreshClipboardMonitoringMode()
            runningAppsService.refresh()
        }
        .onDisappear {
            invalidateSearchRequests()
            bannerTask?.cancel()
            lookupPreviewTask?.cancel()
            webSuggestionTask?.cancel()
            recentURLTask?.cancel()
            keyboardMonitor.stop()
            clipboardStore.setMonitoringMode(.background)
        }
        .onChange(of: query) { _, _ in
            // Editing the query dismisses a pending Empty Trash confirmation,
            // mirroring how the kill command clears its pending candidate.
            if pendingEmptyTrashCount != nil {
                pendingEmptyTrashCount = nil
            }
            if !isCommandMode, let cmd = extractInlineCommand(from: query), cmd.hasSpace {
                aiAnswer.cancel()
                enterCommandMode(commandID: cmd.id, prefilledInput: cmd.args)
                return
            }
            previewLookupDefinition(for: query)
            if !isCommandMode {
                if showsHelpScreen {
                    showsHelpScreen = false
                }
                if isClipboardQuery || isPrefixSuggestionQuery || isCommandSuggestionQuery || isTranslationQuery {
                    // These render their own panels (clip history / prefix menu /
                    // command menu / translation), not backend results. Skip the
                    // search + AI answer entirely - otherwise a background AI
                    // activation flips the floating layout and flashes the old
                    // backdrop while typing. Translation only fires on Enter.
                    aiAnswer.cancel()
                    setInitialSelection()
                } else {
                    // Search drives the AI answer card from its completion handler
                    // (it needs the local result count to decide whether to fire).
                    refreshSearchResults()
                }
            } else {
                aiAnswer.cancel()
            }
            // Google autocomplete rows (appended after engine results). Self-gates
            // by mode and the online-features flag; never blocks search.
            refreshWebSuggestions()
            // Previously-opened URLs matching the query (url-history spec).
            refreshRecentURLs()
        }
        .onChange(of: selectedResultID) { _, _ in
            // Load Quick Actions + read their live state for the new selection.
            refreshQuickActions()
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
        .onChange(of: activeCommandID) { _, newID in
            if let newID { appUIState.lastCommandID = newID }
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
            if launcherWindow()?.isVisible == true {
                Logger(subsystem: "noah-code.Look", category: "window-resize")
                    .debug("didResignActiveNotification -> hideLauncherWindow(restorePreviousApp: false)")
                hideLauncherWindow(restorePreviousApp: false)
            }
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
        .onReceive(NotificationCenter.default.publisher(for: .lookOpenPomoRequested)) { _ in
            DispatchQueue.main.async {
                if !isCommandMode {
                    enterCommandMode(commandID: AppConstants.Launcher.Command.pomo, prefilledInput: "")
                } else {
                    activeCommandID = AppConstants.Launcher.Command.pomo
                    selectedCommandID = AppConstants.Launcher.Command.pomo
                }
            }
        }
    }

    @ViewBuilder
    private func borderedPanel(windowCornerRadius: CGFloat, contentSpacing: CGFloat, contentPadding: CGFloat) -> some View {
        ZStack {
            // When the content floats free (floating panes, or resting on an empty
            // query) the blur + tint backdrop box is dropped so the tiles sit on
            // the bare desktop. A background image, if set, is cropped into each
            // tile (see tileBackground) rather than filling the gaps.
            if !barFloatsFree {
                themedBackground
            }

            VStack(alignment: .leading, spacing: contentSpacing) {
                panelContent
            }
            // Tighter top inset so the search bar sits closer to the window's
            // top edge; keep the original padding on the other three sides.
            .padding(.top, max(4, contentPadding - 8))
            .padding(.horizontal, contentPadding)
            .padding(.bottom, contentPadding)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .font(themeStore.uiFont())
            .foregroundStyle(themeStore.fontColor())
            .background(.black.opacity(barFloatsFree ? 0 : 0.16), in: RoundedRectangle(cornerRadius: windowCornerRadius, style: .continuous))
            .contentShape(Rectangle())
            .onTapGesture { focusActiveInput() }
        }
        .coordinateSpace(name: Self.panelCoordinateSpace)
        .background(
            GeometryReader { geo in
                Color.clear
                    .onAppear { panelSize = geo.size }
                    .onChange(of: geo.size) { _, newSize in panelSize = newSize }
            }
        )
        .clipShape(RoundedRectangle(cornerRadius: windowCornerRadius, style: .continuous))
        .overlay { borderOverlay(cornerRadius: windowCornerRadius) }
        .modifier(PanelDecorationsModifier(
            testHint: { testHintOverlay },
            copyright: { copyrightOverlay },
            killBar: { killConfirmationOverlay },
            deleteBar: { deleteConfirmationOverlay }
        ))
        .layoutPriority(1)
    }

    private func searchInputBar(showsBackground: Bool = true) -> some View {
        SearchInputBar(
            text: $query,
            isCommandMode: $isCommandMode,
            isQueryFocused: $isQueryFocused,
            activeCommand: activeCommand,
            themeStore: themeStore,
            showsBackground: showsBackground,
            onSubmit: handleSubmit,
            onExitCommandMode: exitCommandMode
        )
    }

    @ViewBuilder
    private var panelContent: some View {
        if appUIState.showsThemeSettings {
            ThemeSettingsView(settings: $themeStore.settings)
        } else {
            if !isCommandMode && !showsHelpScreen {
                if shouldShowRunningAppsStrip {
                    // The search field and running-apps icons always share one
                    // background so they read as a single unified bar: a frosted
                    // tile when floating, the classic rounded fill otherwise. The
                    // search field drops its own box since the bar supplies it.
                    topRowBar {
                        HStack(alignment: .center, spacing: 10) {
                            searchInputBar(showsBackground: false)
                                .frame(maxWidth: .infinity)
                            RunningAppsStripView(
                                service: runningAppsService,
                                themeStore: themeStore,
                                onActivate: { key in _ = activateRunningApp(forKey: key) }
                            )
                            .frame(maxWidth: .infinity)
                        }
                    }
                } else {
                    // No running apps: the search field is the whole bar; the
                    // chrome (classic fill or frosted tile) comes from topRowBar.
                    topRowBar { searchInputBar(showsBackground: false) }
                }
            }

            if let bannerMessage {
                bannerView(message: bannerMessage)
            }

            if isCommandMode {
                commandModeView
            } else if isTranslationQuery {
                floatingPanel {
                    LookupDefinitionPanelView(
                        definition: lookupDefinition,
                        emptyHint: translationEmptyHint,
                        isWebMode: isWebTranslationQuery,
                        themeStore: themeStore
                    )
                }
            } else if showsHelpScreen {
                LauncherHelpScreenView(themeStore: themeStore)
            } else if isClipboardQuery && displayedResults.isEmpty {
                // The empty clipboard screen is naturally two columns (history /
                // how-to), so float it as the same two-card grid as the results.
                if showsFloatingCards {
                    twoPaneGrid(hasRight: true) {
                        ClipboardEmptyInfoView(themeStore: themeStore)
                    } right: {
                        ClipboardEmptyHelpView(themeStore: themeStore)
                    }
                } else {
                    ClipboardEmptyStateView(themeStore: themeStore)
                }
            } else if isRecentQuery && displayedResults.isEmpty {
                floatingPanel { RecentEmptyStateView(themeStore: themeStore) }
            } else if hidesResultsForEmptyQuery {
                // Empty query while floating: rest with just the top bar, nothing
                // below. A spacer keeps the bar pinned to the top.
                Spacer(minLength: 0)
            } else {
                resultsRow
            }

            if isCommandMode {
                Spacer(minLength: 0)
            }

            // The hint bar lives inside the left card only on the floating results
            // grid; every other state (classic, translation, empty panels) keeps
            // the full-width bar below.
            if !showsFloatingGrid && !hidesResultsForEmptyQuery && !isKillConfirmationVisible && !isDeleteConfirmationVisible {
                HintBar(hint: currentHint, todo: todoQuickView, themeStore: themeStore)
            }
        }
    }

    @ViewBuilder
    private func bannerView(message: String) -> some View {
        HStack(spacing: 8) {
            Text(message)
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

    @ViewBuilder
    private var resultsRow: some View {
        if aiAnswer.isActive {
            if displayedResults.isEmpty {
                aiAnswerOnlyRow
            } else if backendFilteredResults.isEmpty {
                aiKnowledgeLookupRow
            } else {
                aiAnswerWithResultsRow
            }
        } else {
            resultsListAndPreview
        }
    }

    /// Answer fills the panel (no rows underneath). Floats as a single card that
    /// also carries the hint footer.
    @ViewBuilder
    private var aiAnswerOnlyRow: some View {
        if showsFloatingCards {
            twoPaneGrid(hasRight: false) {
                AIAnswerCardView(controller: aiAnswer, themeStore: themeStore)
            } right: {
                EmptyView()
            }
        } else {
            AIAnswerCardView(controller: aiAnswer, themeStore: themeStore)
                .frame(maxHeight: .infinity)
        }
    }

    /// Knowledge lookup: AI answer beside the web-suggestion list. Floats as the
    /// same two-card grid as the app results (answer holds the hints, suggestions
    /// hold the copyright).
    @ViewBuilder
    private var aiKnowledgeLookupRow: some View {
        if showsFloatingCards {
            twoPaneGrid(hasRight: true) {
                AIAnswerCardView(controller: aiAnswer, themeStore: themeStore)
            } right: {
                ResultsListView(
                    results: displayedResults,
                    selectedID: selectedResultID,
                    pickedKeys: Set(pickedKeys),
                    themeStore: themeStore,
                    onSelect: { selectedResultID = $0 },
                    onOpen: { _ in openSelectedApp() }
                )
            }
        } else {
            // Answer on the left at a comfortable reading measure; suggestion list
            // pinned to a fixed-width column on the right.
            HStack(alignment: .top, spacing: 8) {
                AIAnswerCardView(controller: aiAnswer, themeStore: themeStore)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                resultsListAndPreview
                    .frame(width: AppConstants.Launcher.aiAnswerSuggestionColumnWidth)
                    .frame(maxHeight: .infinity, alignment: .top)
            }
            .frame(maxHeight: .infinity)
        }
    }

    /// Local file/app results coexist with the answer: answer capped on top so the
    /// results grid below keeps its full-width preview pane (and its in-card hints).
    @ViewBuilder
    private var aiAnswerWithResultsRow: some View {
        VStack(spacing: showsFloatingCards ? innerGap : 8) {
            aiAnswerCard
                .frame(maxHeight: 240)
            resultsListAndPreview
        }
    }

    /// The AI answer as a frosted floating tile (plain card when not floating).
    @ViewBuilder
    private var aiAnswerCard: some View {
        if showsFloatingCards {
            paneCard(padding: 6) {
                AIAnswerCardView(controller: aiAnswer, themeStore: themeStore)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        } else {
            AIAnswerCardView(controller: aiAnswer, themeStore: themeStore)
        }
    }

    @ViewBuilder
    private var resultsListAndPreview: some View {
        twoPaneGrid(hasRight: hasRightPane) {
            ResultsListView(
                results: displayedResults,
                selectedID: selectedResultID,
                pickedKeys: Set(pickedKeys),
                themeStore: themeStore,
                onSelect: { selectedResultID = $0 },
                onOpen: { _ in openSelectedApp() }
            )
        } right: {
            if !pickedKeys.isEmpty {
                PickedItemsPanel(
                    pickedKeys: pickedKeys,
                    pickedByKey: pickedResultsByKey,
                    themeStore: themeStore,
                    onRemove: { removePicked(key: $0) },
                    onClearAll: { clearAllPicked() },
                    onOpenAll: { openAllPicked() }
                )
            } else if let selectedResult = previewResult {
                // Info + actions panel: the preview plus any Quick Actions.
                ResultPreviewView(
                    result: selectedResult,
                    quickActions: quickActionDescriptors,
                    quickActionStates: quickActionStates,
                    onRunQuickAction: { descriptor, intent in
                        runQuickAction(descriptor, intent: intent)
                    },
                    onDeleteClipboard: selectedResult.kind == .clipboard
                        ? { deleteClipboardResult(resultID: selectedResult.id) }
                        : nil
                )
            }
        }
    }

    /// The two-card home grid shared by the results screen and the clipboard
    /// empty state: a left card and an optional right card, separated by the
    /// inner gap when floating or a hairline when not. On the floating grid the
    /// hint bar lives in the left card and the copyright in the right.
    @ViewBuilder
    private func twoPaneGrid<L: View, R: View>(
        hasRight: Bool,
        @ViewBuilder left: () -> L,
        @ViewBuilder right: () -> R
    ) -> some View {
        HStack(spacing: showsFloatingCards ? innerGap : 0) {
            paneCard(padding: showsFloatingCards ? 6 : 0) {
                leftPaneCardBody(hasRight: hasRight) { left() }
            }

            if hasRight {
                if !showsFloatingCards { resultsDivider }
                paneCard(padding: showsFloatingCards ? 6 : 0) {
                    rightPaneCardBody { right() }
                }
            }
        }
    }

    /// Left card contents plus, when floating, the hint footer (with the
    /// copyright appended if there is no right card to hold it).
    @ViewBuilder
    private func leftPaneCardBody<Content: View>(hasRight: Bool, @ViewBuilder _ content: () -> Content) -> some View {
        if showsFloatingCards {
            VStack(alignment: .leading, spacing: 0) {
                content()
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                cardFooter {
                    HintBar(hint: currentHint, todo: todoQuickView, themeStore: themeStore)
                    Spacer(minLength: 8)
                    // No right card → copyright has nowhere else to go.
                    if !hasRight { copyrightLink }
                }
            }
        } else {
            content()
        }
    }

    /// Right card contents plus, when floating, the copyright footer.
    @ViewBuilder
    private func rightPaneCardBody<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        if showsFloatingCards {
            VStack(alignment: .leading, spacing: 0) {
                content()
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                cardFooter {
                    Spacer(minLength: 0)
                    copyrightLink
                }
            }
        } else {
            content()
        }
    }

    /// A thin footer strip inside a floating card holding a slice of the old
    /// full-width hint bar.
    @ViewBuilder
    private func cardFooter<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        HStack(spacing: 0) {
            content()
        }
        .padding(.horizontal, 6)
        .padding(.top, 6)
        .padding(.bottom, 2)
    }

    /// i3-style inner gap between the three home panes (0 = classic flat layout).
    private var innerGap: CGFloat { CGFloat(themeStore.settings.innerGap) }
    private var usesPanes: Bool { innerGap > 0 }

    /// True when the carded results screen is showing with the gap on: the panes
    /// become self-contained frosted tiles floating on the bare desktop, so the
    /// window backdrop and the full-width hint/copyright strip are dropped. Other
    /// screens (command mode, settings, help, empty states) keep the backdrop.
    /// Gate for the floating layout. Kept intentionally CHEAP and STABLE: it
    /// depends only on the coarse mode (gap on, not command/settings/help/AI),
    /// never on the query text or the live result count. That stability matters -
    /// this decides whether the expensive window + per-card blur views exist, so
    /// letting it flip per keystroke (e.g. as clipboard/translation results stream
    /// in) churned NSVisualEffectViews on the main thread and froze typing.
    private var showsFloatingCards: Bool {
        usesPanes
            && !isCommandMode
            && !appUIState.showsThemeSettings
            && !showsHelpScreen
    }

    /// True when the floating content is the two-card grid (results or clipboard
    /// empty) that carries its hint + copyright inside the cards. Translation and
    /// the recent-empty state float as a single card and keep the bottom bar.
    /// Only gates a `Text`, so it may read live state.
    private var showsFloatingGrid: Bool {
        showsFloatingCards
            && !isTranslationQuery
            && !(isRecentQuery && displayedResults.isEmpty)
    }

    private var isQueryEmpty: Bool {
        query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// With an empty query on the home screen, rest as just the search bar - hide
    /// the results columns and the hint bar below it. Applies in both modes (gap
    /// or no gap), so an empty launcher is always just the top bar.
    private var hidesResultsForEmptyQuery: Bool {
        isQueryEmpty
            && !isCommandMode
            && !appUIState.showsThemeSettings
            && !showsHelpScreen
    }

    /// True whenever the panel has no backdrop box and its content floats freely
    /// on the desktop: either the panes are floating, or we're resting on an empty
    /// query. In both cases the top bar becomes a self-contained frosted tile so
    /// it stays legible on the bare desktop.
    private var barFloatsFree: Bool {
        showsFloatingCards || hidesResultsForEmptyQuery
    }

    /// Wraps a home-screen pane in its own rounded, frosted card so the inner gap
    /// reads as real separation between "windows". A no-op when the gap is 0,
    /// preserving the classic flat layout exactly. Each card carries its own blur
    /// so it stays legible even once the window backdrop is removed.
    @ViewBuilder
    private func paneCard(padding: CGFloat, @ViewBuilder _ content: () -> some View) -> some View {
        if barFloatsFree {
            content()
                .padding(padding)
                .background { tileBackground(cornerRadius: 12, floats: true) }
                .overlay { tileBorder(cornerRadius: 12) }
                // Lift each pane off the backdrop so the three parts read as
                // separate floating tiles rather than sections of one box.
                .shadow(color: .black.opacity(0.25), radius: 7, x: 0, y: 3)
        } else {
            content()
        }
    }

    /// The frosted surface shared by every floating tile (top bar + columns). When
    /// a background image is set, each tile shows its own aligned slice of that
    /// image (cropped to the tile's window position) instead of a blurred desktop,
    /// so the tiles read as separate windows onto one image. A dark scrim + tint
    /// on top keeps the tile content legible.
    @ViewBuilder
    private func tileBackground(cornerRadius: CGFloat, floats: Bool) -> some View {
        if floats {
            ZStack {
                if let image = themeStore.backgroundImage {
                    croppedBackgroundImage(image)
                } else {
                    VisualEffectBlur(material: themeStore.settings.blurMaterial.material)
                }
                Color.black.opacity(0.30)
                themeStore.controlFillColor()
            }
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
        } else {
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .fill(themeStore.controlFillColor())
        }
    }

    /// The outline for a floating tile, drawn with the user's configured theme
    /// border (color + thickness from Settings). Nothing is drawn when the border
    /// thickness is 0, matching the rest of the app.
    @ViewBuilder
    private func tileBorder(cornerRadius: CGFloat) -> some View {
        let width = themeStore.borderLineWidth()
        if width > 0 {
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .strokeBorder(themeStore.borderColor(), lineWidth: width)
        }
    }

    /// The slice of the full-panel background image that sits behind this tile:
    /// the image is sized to the whole panel and offset by the tile's origin in
    /// the panel coordinate space, so adjacent tiles show a continuous image cut
    /// apart by the gaps.
    @ViewBuilder
    private func croppedBackgroundImage(_ image: NSImage) -> some View {
        GeometryReader { tileGeo in
            let frame = tileGeo.frame(in: .named(Self.panelCoordinateSpace))
            backgroundImageView(image: image)
                .frame(width: panelSize.width, height: panelSize.height)
                .offset(x: -frame.minX, y: -frame.minY)
                .blur(radius: themeStore.settings.backgroundImageBlur)
        }
    }

    /// Background wrapper for the unified top row (search + running apps). A
    /// frosted floating tile when the panes are floating, otherwise the classic
    /// rounded search-bar fill - so the merged bar looks consistent on every
    /// screen, gap or no gap.
    private func topRowBar<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        // Apply the chrome as a STABLE modifier chain (background/overlay/shadow
        // always present, only their values change) - never an if/else that swaps
        // the subtree - so the search field keeps its identity and its keyboard
        // focus when the bar flips between the classic fill and the frosted tile
        // (e.g. typing the first character out of the empty-rest state at gap 0).
        let floats = barFloatsFree
        return content()
            .background { tileBackground(cornerRadius: floats ? 12 : 10, floats: floats) }
            .overlay {
                if floats {
                    tileBorder(cornerRadius: 12)
                }
            }
            .shadow(color: floats ? .black.opacity(0.25) : .clear,
                    radius: floats ? 7 : 0, x: 0, y: floats ? 3 : 0)
    }

    /// Wraps a single-panel home state (translation, clipboard/recent empty) in a
    /// frosted floating card when floating, so it keeps a background once the
    /// window backdrop is removed. A no-op otherwise.
    @ViewBuilder
    private func floatingPanel<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        if showsFloatingCards {
            paneCard(padding: 0) {
                content()
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        } else {
            content()
        }
    }

    /// The selected result eligible for the right-hand preview pane, or nil when
    /// the list should span full width (suggestions, nothing selected, etc.).
    private var previewResult: LauncherResult? {
        guard pickedKeys.isEmpty,
              !isPrefixSuggestionQuery, !isCommandSuggestionQuery,
              let selectedID = selectedResultID,
              let selectedResult = displayedResults.first(where: { $0.id == selectedID }),
              AppConstants.Launcher.WebSuggestion.text(fromResultID: selectedResult.id) == nil,
              AppConstants.Launcher.WebURL.url(fromResultID: selectedResult.id) == nil
        else { return nil }
        return selectedResult
    }

    /// Whether a right-hand pane (picked list or preview) is currently shown.
    private var hasRightPane: Bool { !pickedKeys.isEmpty || previewResult != nil }

    private var copyrightLink: some View {
        Link("© 2026 by Kunkka", destination: URL(string: "https://github.com/kunkka19xx")!)
            .font(themeStore.uiFont(size: CGFloat(max(9, themeStore.settings.fontSize - 4)), weight: .regular))
            .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.50))
    }

    private var resultsDivider: some View {
        Rectangle()
            .fill(.white.opacity(0.08))
            .frame(width: 1)
            .padding(.vertical, 4)
    }

    @ViewBuilder
    private func borderOverlay(cornerRadius: CGFloat) -> some View {
        let borderWidth = themeStore.borderLineWidth()
        // When the content floats free the panes sit on the bare desktop, so drop
        // the outer window outline (keep it only to surface the sudo warning).
        if borderWidth > 0 && (!barFloatsFree || hasSudoWarning) {
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .strokeBorder(
                    hasSudoWarning ? Color.orange.opacity(0.95) : themeStore.borderColor(),
                    lineWidth: borderWidth
                )
        }
    }

    @ViewBuilder
    private var testHintOverlay: some View {
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

    @ViewBuilder
    private var copyrightOverlay: some View {
        // On the floating results grid the copyright moves into the right-hand
        // card footer; on the empty-rest screen it's hidden entirely; otherwise it
        // stays in the panel's bottom-right corner.
        if !showsFloatingGrid && !hidesResultsForEmptyQuery {
            copyrightLink
                .padding(.trailing, 10)
                .padding(.bottom, 8)
        }
    }

    @ViewBuilder
    private var killConfirmationOverlay: some View {
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

    @ViewBuilder
    private var deleteConfirmationOverlay: some View {
        if !isCommandMode, let pendingEmptyTrashCount {
            EmptyTrashConfirmationBar(
                itemCount: pendingEmptyTrashCount,
                themeStore: themeStore,
                onConfirm: { confirmDeleteSelection() },
                onCancel: { cancelDeleteSelection() }
            )
            .padding(.horizontal, 14)
            .padding(.bottom, 24)
        }
    }


}

private struct PanelDecorationsModifier<TestHint: View, Copyright: View, KillBar: View, DeleteBar: View>: ViewModifier {
    @ViewBuilder let testHint: () -> TestHint
    @ViewBuilder let copyright: () -> Copyright
    @ViewBuilder let killBar: () -> KillBar
    @ViewBuilder let deleteBar: () -> DeleteBar

    func body(content: Content) -> some View {
        content
            .overlay(alignment: .topTrailing, content: testHint)
            .overlay(alignment: .bottomTrailing, content: copyright)
            .overlay(alignment: .bottom, content: killBar)
            .overlay(alignment: .bottom, content: deleteBar)
    }
}
