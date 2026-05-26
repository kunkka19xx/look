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

    func performWebSearchFromQuery() {
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
