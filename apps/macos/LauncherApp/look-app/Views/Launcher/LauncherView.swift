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

    @State var query = ""
    @State var commandInput = ""
    @State var isCommandMode = false
    @State var backendResults: [LauncherResult] = []
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
    /// On success the launcher is *not* hidden here — instead we let
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

            let folderPath = URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent(entry.relativePath)
                .path
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

    var clipboardSearchTerm: String? {
        LauncherClipboardFeature.searchTerm(from: query)
    }

    var clipboardResults: [LauncherResult] {
        guard let clipboardSearchTerm else { return [] }

        return clipboardStore.search(clipboardSearchTerm).map { entry in
            LauncherClipboardFeature.makeResult(entry: entry, dateFormatter: Self.clipboardSubtitleDateFormatter)
        }
    }

    var displayedResults: [LauncherResult] {
        isClipboardQuery ? clipboardResults : backendFilteredResults
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
                return ["Esc back", "Tab/Cmd+1-5 switch", "Cmd+/ command mode", "Cmd+Shift+, settings"]
            }
            if activeCommandID == AppConstants.Launcher.Command.pomo {
                return ["Space start/pause", "R reset", "P music", "Esc back", "Tab/Cmd+1-5 switch"]
            }
            return ["Enter run", "Tab select", "Cmd+1-5 switch", "Esc back"]
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

        return ["Enter open", "Cmd+F reveal", "Cmd+D trash", "Cmd+H help", "Cmd+/ command mode"]
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
        let contentSpacing: CGFloat = isCommandMode ? 8 : 12
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
                enterCommandMode(commandID: cmd.id, prefilledInput: cmd.args)
                return
            }
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
                let log = Logger(subsystem: "noah-code.Look", category: "window-resize")
                // Dragging the launcher across screens triggers a
                // programmatic resize, which briefly drops Look's frontmost
                // status and fires this notification. Ignore the auto-hide
                // when that just happened — the user is still interacting
                // with the launcher, not switching to another app.
                if WindowAutoScale.didProgrammaticallyResizeRecently() {
                    log.debug("didResignActiveNotification ignored (within \(WindowAutoScale.resizeSettleWindow, privacy: .public)s of a programmatic resize)")
                } else {
                    log.debug("didResignActiveNotification -> hideLauncherWindow(restorePreviousApp: false)")
                    hideLauncherWindow(restorePreviousApp: false)
                }
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
            themedBackground

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
            .background(.black.opacity(0.16), in: RoundedRectangle(cornerRadius: windowCornerRadius, style: .continuous))
            .contentShape(Rectangle())
            .onTapGesture { focusActiveInput() }
        }
        .background(Color.clear)
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

    @ViewBuilder
    private var searchInputBar: some View {
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

    @ViewBuilder
    private var panelContent: some View {
        if appUIState.showsThemeSettings {
            ThemeSettingsView(settings: $themeStore.settings)
        } else {
            if !isCommandMode && !showsHelpScreen {
                if shouldShowRunningAppsStrip {
                    // Split the search-bar row in half: search field on the
                    // left, running-apps icons on the right. No floating strip,
                    // no window resize — toggled via Settings → Running Apps.
                    HStack(alignment: .center, spacing: 10) {
                        searchInputBar
                            .frame(maxWidth: .infinity)
                        RunningAppsStripView(
                            service: runningAppsService,
                            themeStore: themeStore,
                            onActivate: { key in _ = activateRunningApp(forKey: key) }
                        )
                        .frame(maxWidth: .infinity)
                    }
                } else {
                    searchInputBar
                }
            }

            if let bannerMessage {
                bannerView(message: bannerMessage)
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
            } else if showsHelpScreen {
                LauncherHelpScreenView(themeStore: themeStore)
            } else if isClipboardQuery && displayedResults.isEmpty {
                ClipboardEmptyStateView(themeStore: themeStore)
            } else {
                resultsRow
            }

            if isCommandMode {
                Spacer(minLength: 0)
            }

            if !isKillConfirmationVisible && !isDeleteConfirmationVisible {
                HintBar(hint: currentHint, themeStore: themeStore)
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
        HStack(spacing: 0) {
            ResultsListView(
                results: displayedResults,
                selectedID: selectedResultID,
                pickedKeys: Set(pickedKeys),
                themeStore: themeStore,
                onSelect: { selectedResultID = $0 },
                onOpen: { _ in openSelectedApp() }
            )

            if !pickedKeys.isEmpty {
                resultsDivider
                PickedItemsPanel(
                    pickedKeys: pickedKeys,
                    pickedByKey: pickedResultsByKey,
                    themeStore: themeStore,
                    onRemove: { removePicked(key: $0) },
                    onClearAll: { clearAllPicked() }
                )
            } else if let selectedID = selectedResultID,
                      let selectedResult = displayedResults.first(where: { $0.id == selectedID }) {
                resultsDivider
                ResultPreviewView(
                    result: selectedResult,
                    onDeleteClipboard: selectedResult.kind == .clipboard
                        ? { deleteClipboardResult(resultID: selectedResult.id) }
                        : nil
                )
            }
        }
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
        if borderWidth > 0 {
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

    private var copyrightOverlay: some View {
        Link("© 2026 by Kunkka", destination: URL(string: "https://github.com/kunkka19xx")!)
            .font(themeStore.uiFont(size: CGFloat(max(9, themeStore.settings.fontSize - 4)), weight: .regular))
            .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.50))
            .padding(.trailing, 10)
            .padding(.bottom, 8)
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
