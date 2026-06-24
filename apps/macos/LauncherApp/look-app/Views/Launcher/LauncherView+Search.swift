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
        guard !isClipboardQuery else {
            invalidateSearchRequests()
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

            // Fast path: search the raw query first and paint immediately. The
            // on-device model is never in front of results — it only refines.
            let rawResults = await Task.detached(priority: .userInitiated) {
                bridge.search(query: currentQuery, limit: searchLimit)
            }.value
            guard !Task.isCancelled else { return }
            publishSearchResults(rawResults, searchID: searchID, for: currentQuery)

            // Rescue pass: only when AI is on AND the raw query found nothing,
            // let the model rewrite the natural-language query into the engine's
            // prefix grammar and re-search. We never run this when raw results
            // exist — a rewrite that narrows "firefox" to apps-only would wrongly
            // drop the matching folder/files the user can already see.
            guard aiEnabled, rawResults.isEmpty else { return }
            guard let rewritten = await AIQueryRouter.shared.rewrite(
                query: currentQuery,
                using: aiProvider
            ), rewritten != currentQuery else { return }
            guard !Task.isCancelled else { return }

            let refined = await Task.detached(priority: .userInitiated) {
                bridge.search(query: rewritten, limit: searchLimit)
            }.value
            guard !Task.isCancelled, !refined.isEmpty else { return }
            publishSearchResults(refined, searchID: searchID, for: currentQuery)
        }
    }

    /// Publishes results on the main actor only if this request is still the
    /// latest and the query hasn't changed out from under it.
    @MainActor
    private func publishSearchResults(
        _ results: [LauncherResult],
        searchID: UInt64,
        for requestedQuery: String
    ) {
        guard searchID == latestSearchID else { return }
        guard !isCommandMode, query == requestedQuery else { return }
        backendResults = results
        setInitialSelection()

        // Additive AI answer card. Driven from here so it knows the local result
        // count — a multi-word query with no local match is treated as a
        // knowledge lookup. Self-gates; never blocks search.
        aiAnswer.update(
            query: requestedQuery,
            resultCount: results.count,
            aiEnabled: themeStore.settings.aiEnabled,
            provider: themeStore.settings.aiProvider
        )
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
    /// only for plain text queries while online features are on. Self-gating —
    /// clears the rows in any non-applicable mode.
    func refreshWebSuggestions() {
        webSuggestionTask?.cancel()
        let currentQuery = query
        let trimmed = currentQuery.trimmingCharacters(in: .whitespacesAndNewlines)

        guard themeStore.settings.aiEnabled,
              !isCommandMode, !isClipboardQuery, !isPrefixSuggestionQuery,
              !isCommandSuggestionQuery, !isTranslationQuery, trimmed.count >= 2
        else {
            if !webSuggestions.isEmpty { webSuggestions = [] }
            return
        }

        webSuggestionTask = Task {
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            guard !Task.isCancelled else { return }
            let suggestions = await WebSuggestionService.suggestions(
                for: currentQuery,
                limit: AppConstants.Launcher.WebSuggestion.limit
            )
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard query == currentQuery, !isCommandMode else { return }
                webSuggestions = suggestions
                // These rows arrive after publishSearchResults() already seeded
                // selection. For a query with no local results selection is nil,
                // so re-seed it onto the first suggestion, otherwise Enter on a
                // suggestion-only list does nothing.
                if selectedResultID == nil {
                    setInitialSelection()
                }
            }
        }
    }

    func reloadConfig() {
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
            }
        }
    }
}
