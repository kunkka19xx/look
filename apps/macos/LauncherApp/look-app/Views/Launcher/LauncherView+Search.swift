import AppKit
import SwiftUI

extension LauncherView {
    func invalidateSearchRequests() {
        latestSearchID &+= 1
        searchTask?.cancel()
        searchTask = nil
    }

    func beginSearchRequest() -> UInt64 {
        latestSearchID &+= 1
        return latestSearchID
    }

    func refreshSearchResults() {
        guard !isCommandMode else { return }
        fileRecallEmptyMessage = nil
        fileRecallNote = nil
        mainBarAction = nil
        guard !isClipboardQuery else {
            invalidateSearchRequests()
            setInitialSelection()
            return
        }
        // The rest screen shows no rows, and the engine answers an empty query
        // by scoring the whole index. Clear instead of searching for nothing.
        guard !hidesResultsForEmptyQuery else {
            invalidateSearchRequests()
            backendResults = []
            setInitialSelection()
            return
        }

        let currentQuery = query
        let searchLimit = AppConstants.Launcher.defaultSearchLimit
        let aiEnabled = themeStore.settings.aiEnabled
        let aiProvider = themeStore.settings.aiProvider
        let searchID = beginSearchRequest()
        searchTask?.cancel()
        searchTask = Task {
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            guard !Task.isCancelled else { return }

            // File-recall auto-detect: "files I downloaded yesterday", "pdfs this
            // week", "screenshots today". Runs against Look's own index; nil means
            // it was not a file-recall query, so normal search proceeds below.
            let fileOutcome = await Task.detached(priority: .userInitiated) {
                bridge.searchFiles(query: currentQuery, limit: searchLimit)
            }.value
            if let fileOutcome {
                guard !Task.isCancelled else { return }
                publishSearchResults(
                    fileOutcome.results, searchID: searchID, for: currentQuery,
                    isFileRecall: true, relaxed: fileOutcome.relaxed)
                return
            }

            // Fast path: search the raw query first and paint immediately. The
            // on-device model is never in front of results - it only refines.
            let rawResults = await Task.detached(priority: .userInitiated) {
                bridge.search(query: currentQuery, limit: searchLimit)
            }.value
            guard !Task.isCancelled else { return }
            publishSearchResults(rawResults, searchID: searchID, for: currentQuery)

            // Rescue pass: only when AI is on AND the raw query found nothing,
            // the planner interprets the phrasing into a STRUCTURED file query
            // (never a string rewrite), executed through the same engine path
            // as the deterministic parser and labeled as interpreted. We never
            // run this when raw results exist - an interpretation that narrows
            // "firefox" would wrongly drop matches the user can already see.
            guard aiEnabled, rawResults.isEmpty else { return }
            _ = aiProvider
            let calls = await ActionPlanner().plan(query: currentQuery)
            guard !Task.isCancelled else { return }
            // The main bar confirms ONE row, so a compound plan belongs to the
            // AI panel's multi-step bar. Running just the first step here would
            // do less than the user asked for, with no sign of the rest:
            // Enter escalates instead, exactly like an unresolvable call.
            guard calls.count == 1, let call = calls.first else { return }
            if call.toolID != "files.recall" {
                // Action-shaped phrasing: the Q&A answer card must not "refuse"
                // on our behalf. The resolved action becomes the FIRST, selected
                // result row - the visible row is the confirmation, one Enter
                // performs it. Unresolvable calls keep the Enter-escalation
                // fallback to the AI surface.
                guard searchID == latestSearchID, query == currentQuery else { return }
                aiAnswer.cancel()
                if let planned = actionController.quickAction(for: call) {
                    mainBarAction = planned
                    selectedResultID = AppConstants.Launcher.AIAction.resultID(
                        toolID: planned.toolID)
                }
                return
            }
            let interpreted = await Task.detached(priority: .userInitiated) {
                bridge.searchFiles(params: call.params, limit: searchLimit)
            }.value
            guard !Task.isCancelled, let interpreted, !interpreted.results.isEmpty else { return }
            publishSearchResults(
                interpreted.results, searchID: searchID, for: currentQuery,
                isFileRecall: true, relaxed: interpreted.relaxed,
                interpreted: Self.recallSummary(call.params))
        }
    }

    /// Re-runs every `run` block, then reindexes so the rows it produced are
    /// picked up.
    ///
    /// Off the main actor and after the config reload, not inline with it: each
    /// block spawns a command and waits for it, so doing this on the main actor
    /// freezes the window for the sum of their timeouts. The reindex is
    /// triggered from the completion, which keeps the ordering that matters -
    /// the blocks must run before the pass that reads their rows.
    private func refreshRunBlocksInBackground() {
        Task {
            let outcome = await Task.detached(priority: .utility) {
                let outcome = EngineBridge.shared.refreshRunBlocks()
                // Only worth a second index pass when a block actually produced
                // rows; the reload above already covered everything else.
                if outcome.refreshed > 0 {
                    _ = EngineBridge.shared.requestIndexRefresh()
                }
                return outcome
            }.value

            await MainActor.run {
                for failure in outcome.errors {
                    showBanner(failure, style: .error, duration: 3.0)
                }
            }
        }
    }

    /// Publishes results on the main actor only if this request is still the
    /// latest and the query hasn't changed out from under it.
    @MainActor
    private func publishSearchResults(
        _ results: [LauncherResult],
        searchID: UInt64,
        for requestedQuery: String,
        isFileRecall: Bool = false,
        relaxed: String? = nil,
        interpreted: String? = nil
    ) {
        guard searchID == latestSearchID else { return }
        guard !isCommandMode, query == requestedQuery else { return }
        backendResults = results
        setInitialSelection()

        // A detected file-recall query owns the panel: on zero matches show an
        // honest "no files" state instead of the knowledge-lookup AI card;
        // results from a relaxed retry or a model interpretation say so.
        if isFileRecall {
            fileRecallEmptyMessage = results.isEmpty
                ? "No files match \u{201C}\(requestedQuery)\u{201D}."
                : nil
            let note = [
                interpreted.map { "Interpreted: \($0)" },
                Self.relaxationNote(relaxed),
            ].compactMap { $0 }.joined(separator: "  ·  ")
            fileRecallNote = (results.isEmpty || note.isEmpty) ? nil : note
            aiAnswer.cancel()
            return
        }
        fileRecallEmptyMessage = nil
        fileRecallNote = nil

        // Additive AI answer card. Driven from here so it knows the local result
        // count - a multi-word query with no local match is treated as a
        // knowledge lookup. Self-gates; never blocks search.
        aiAnswer.update(
            query: requestedQuery,
            resultCount: results.count,
            aiEnabled: themeStore.settings.aiEnabled,
            provider: themeStore.settings.aiProvider
        )
    }

    /// A file recall handed over from the AI panel: leave AI mode and show the
    /// results in the main panel, which owns file rows (open/reveal/quicklook).
    /// Empty params = the router's deterministic `files` decision (re-parse the
    /// raw query); non-empty = the model's structured `recall` step, labeled as
    /// interpreted so the user sees the model read their phrasing.
    func runModelRecall(_ request: ActionController.RecallRequest) {
        chat.endSession()
        isAIMode = false

        if request.params.isEmpty {
            query = request.query
            DispatchQueue.main.async {
                refreshSearchResults()
                isQueryFocused = true
            }
            return
        }

        // Supersede any in-flight search, then run this one exactly like the
        // others: off the main thread (the FFI call scans the index and would
        // otherwise freeze the window), and published through the shared path
        // so it gets the same searchID + query-identity guard.
        let searchID = beginSearchRequest()
        searchTask?.cancel()
        querySilentlyCleared = true
        query = request.query
        aiAnswer.cancel()

        // Focus first: the field must be usable while the index is queried,
        // not only once results land.
        DispatchQueue.main.async { isQueryFocused = true }

        let params = request.params
        let requested = request.query
        let limit = AppConstants.Launcher.defaultSearchLimit
        searchTask = Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.searchFiles(params: params, limit: limit)
            }.value
            guard !Task.isCancelled else { return }
            publishSearchResults(
                outcome?.results ?? [], searchID: searchID, for: requested,
                isFileRecall: true, relaxed: outcome?.relaxed,
                interpreted: Self.recallSummary(params))
        }
    }

    /// One-Enter action from the main-bar row: perform, banner with undo, back
    /// to a clean bar. No AI-mode detour - the row already showed the plan.
    func runQuickAction(_ planned: PlannedAction) {
        mainBarAction = nil
        clearQuerySilently()
        Task {
            do {
                let summary = try await actionController.performQuickAction(planned)
                showBanner("\(summary)  ·  ⌘Z undo", duration: 3.0)
            } catch {
                showBanner("Failed: \(error.localizedDescription)", style: .error, duration: 4.0)
            }
        }
    }

    /// Compact "what the model understood" summary: "pdf · downloads · last week".
    private static func recallSummary(_ params: [String: String]) -> String {
        ["terms", "types", "location", "when"]
            .compactMap { key in params[key].flatMap { $0.isEmpty ? nil : $0 } }
            .joined(separator: " ")
    }

    /// Human text for the engine's file-recall relaxation codes.
    private static func relaxationNote(_ relaxed: String?) -> String? {
        switch relaxed {
        case "window":
            return "Nothing in that exact timeframe - showing slightly older matches."
        case "terms":
            return "Some words didn't match anything - showing everything else that fits."
        case "window_terms":
            return "No exact matches - showing the closest recent files."
        default:
            return nil
        }
    }

    func performWebSearchFromQuery() {
        guard !isCommandMode else { return }
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        if let translationCommand = extractTranslationQuery(from: trimmed) {
            handleTranslation(command: translationCommand)
            isQueryFocused = true
            return
        }

        performWebSearch(for: trimmed)
    }

    /// Opens a Google search for `text` in the default browser. Shared by the
    /// Cmd+Enter web-search shortcut and the autocomplete suggestion rows.
    func performWebSearch(for text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        var components = URLComponents(string: "https://www.google.com/search")
        components?.queryItems = [URLQueryItem(name: "q", value: trimmed)]
        guard let url = components?.url else { return }
        let config = NSWorkspace.OpenConfiguration()
        config.activates = true
        NSWorkspace.shared.open(url, configuration: config, completionHandler: nil)
    }

    /// Fetches Google autocomplete rows for the current query, debounced, and
    /// only for plain text queries while online features are on. Self-gating -
    /// clears the rows in any non-applicable mode.
    func refreshWebSuggestions() {
        webSuggestionTask?.cancel()
        let currentQuery = query
        let trimmed = currentQuery.trimmingCharacters(in: .whitespacesAndNewlines)

        guard themeStore.settings.aiEnabled,
              !isCommandMode, !isClipboardQuery, !isPrefixSuggestionQuery,
              !isCommandSuggestionQuery, !isTranslationQuery,
              trimmed.count >= AppConstants.Launcher.minSuggestionQueryLength
        else {
            if !webSuggestions.isEmpty { webSuggestions = [] }
            return
        }

        webSuggestionTask = Task {
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            guard !Task.isCancelled else { return }
            let suggestionLimit = AppConstants.Launcher.WebSuggestion.limit
            let suggestions = await Task.detached(priority: .userInitiated) {
                bridge.webSuggestions(query: currentQuery, limit: suggestionLimit)
            }.value
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard query == currentQuery, !isCommandMode else { return }
                webSuggestions = suggestions
                // These rows arrive after publishSearchResults() already seeded
                // selection. For a query with no local results selection is nil,
                // so re-seed it onto the first suggestion, otherwise Enter on a
                // suggestion-only list does nothing. NOT while the answer card
                // is answering: a highlighted Google row would misread as what
                // Enter does; arrow keys still reach the rows deliberately.
                if selectedResultID == nil, !aiAnswer.isActive {
                    setInitialSelection()
                }
            }
        }
    }

    /// Fetches previously-opened URLs matching the current query, debounced and
    /// off the main thread (the lookup opens the DB). Self-gating - clears the
    /// rows in any non-applicable mode. Mirrors `refreshWebSuggestions`.
    func refreshRecentURLs() {
        recentURLTask?.cancel()
        let currentQuery = query
        let trimmed = currentQuery.trimmingCharacters(in: .whitespacesAndNewlines)

        guard allowsSuggestionRows, !isTranslationQuery,
              trimmed.count >= AppConstants.Launcher.minSuggestionQueryLength
        else {
            if !recentURLEntries.isEmpty { recentURLEntries = [] }
            return
        }

        recentURLTask = Task {
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            guard !Task.isCancelled else { return }
            let limit = AppConstants.Launcher.WebURL.recentLimit
            let entries = await Task.detached(priority: .userInitiated) {
                bridge.recentURLs(query: currentQuery, limit: limit)
            }.value
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard query == currentQuery, !isCommandMode else { return }
                recentURLEntries = entries
                if selectedResultID == nil {
                    setInitialSelection()
                }
            }
        }
    }

    /// Callers reloading as a side effect of some other action pass their own
    /// `success*` values rather than posting a second banner, which would replace
    /// any config warnings reported here before the user could read them.
    @discardableResult
    func reloadConfig(
        successMessage: String = "Config reloaded",
        successStyle: BannerStyle = .info,
        successDuration: Double = 2.0
    ) -> Bool {
        let result = themeStore.reloadFromConfig()
        let backendReloaded = bridge.reloadConfig()
        clipboardStore.reloadFromConfig()
        // Declared block icons and `then` targets are cached for the process, so
        // a reload is the point where an edited file should start showing.
        SourceBlockCatalog.invalidate()
        refreshRunBlocksInBackground()

        // Sync settings blur multiplier to AppUIState
        if let blurMultiplier = result.settingsBlurMultiplier {
            appUIState.settingsBlurMultiplier = blurMultiplier
        }

        var message = successMessage
        var style: BannerStyle = successStyle
        var duration: Double = successDuration
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
        return backendReloaded
    }

    func showBanner(
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
                // The banner was the only offer of undo; once it's gone the
                // delete is final (and the buffer shouldn't leak the payload).
                deletedConversation = nil
            }
        }
    }
}
