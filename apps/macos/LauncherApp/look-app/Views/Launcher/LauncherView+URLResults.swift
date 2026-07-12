import Foundation

/// URL-aware result rows (issue #232 + url-history spec): the live "Open <url>"
/// row for a URL-like query, previously-opened URLs matching the query, and the
/// score merge that interleaves them with local results. Detection and frecency
/// scoring live in the Rust core (`classifyURL` / `recentURLs`), shared with
/// linows; this file is only presentation and ordering.
extension LauncherView {
    /// Modes where synthesized suggestion rows (URLs, web search) must not show:
    /// command, clipboard, and the `"`/`:` discovery menus.
    var allowsSuggestionRows: Bool {
        !isCommandMode && !isClipboardQuery && !isPrefixSuggestionQuery && !isCommandSuggestionQuery
    }

    /// Builds a synthesized URL row. Shared by the live row and history rows so
    /// their id/kind/shape stay identical (the open handler keys off the id).
    func makeURLRow(url: String, subtitle: String, score: Int) -> LauncherResult {
        LauncherResult(
            id: AppConstants.Launcher.WebURL.resultID(url: url),
            kind: .app,
            title: url,
            subtitle: subtitle,
            path: url,
            score: score
        )
    }

    /// The live "Open <url>" row for a URL-like query, plus its tier, or nil. The
    /// tier decides placement in `displayedResults`.
    var urlResult: (result: LauncherResult, tier: URLMatch.Tier)? {
        guard allowsSuggestionRows, let match = bridge.classifyURL(query: query) else { return nil }
        let row = makeURLRow(url: match.url, subtitle: AppConstants.Launcher.WebURL.openSubtitle, score: 0)
        return (row, match.tier)
    }

    /// Previously-opened URLs matching the query, as rows carrying the core's
    /// frecency `score`. Cached in `recentURLEntries` by `refreshRecentURLs` (the
    /// lookup does DB I/O, so it can't run in this getter). Deduped against the
    /// live row so the same address never appears twice.
    var recentURLResults: [LauncherResult] {
        guard allowsSuggestionRows else { return [] }
        let liveURL = urlResult?.result.path
        return recentURLEntries.compactMap { entry in
            guard entry.url != liveURL else { return nil }
            return makeURLRow(url: entry.url, subtitle: AppConstants.Launcher.WebURL.recentSubtitle, score: entry.score)
        }
    }

    /// Stable score-descending merge of local results and recent-URL rows, so a
    /// frequently/recently opened URL rises exactly as far as its frecency earns
    /// against local matches (no fixed hit-count cutoff). Both inputs are already
    /// score-sorted; on ties the local result stays ahead, so a URL never
    /// displaces an equally-ranked app/file.
    func mergeByScore(_ local: [LauncherResult], _ recents: [LauncherResult]) -> [LauncherResult] {
        guard !recents.isEmpty else { return local }
        var merged: [LauncherResult] = []
        merged.reserveCapacity(local.count + recents.count)
        var i = 0, j = 0
        while i < local.count, j < recents.count {
            if recents[j].score > local[i].score {
                merged.append(recents[j])
                j += 1
            } else {
                merged.append(local[i])
                i += 1
            }
        }
        merged.append(contentsOf: local[i...])
        merged.append(contentsOf: recents[j...])
        return merged
    }
}
