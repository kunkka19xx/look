import Foundation

enum LauncherPinnedLookupScope: Equatable {
    case unscoped
    case apps
    case files
    case folders
    case disabled
}

enum LauncherSearchLogic {
    static func pinnedLookupScope(for query: String) -> LauncherPinnedLookupScope {
        if AppConstants.Launcher.QueryPrefix.matches(query, prefixName: "r")
            || AppConstants.Launcher.QueryPrefix.matches(query, prefixName: "c")
            || AppConstants.Launcher.QueryPrefix.matches(query, prefixName: "rc")
        {
            // Recent (rc") is engine-ranked by recency; suppress quick-folder and
            // Finder pinned injection so they don't pollute the recent list.
            return .disabled
        }
        if AppConstants.Launcher.QueryPrefix.matches(query, prefixName: "a") {
            return .apps
        }
        if AppConstants.Launcher.QueryPrefix.matches(query, prefixName: "f") {
            return .files
        }
        if AppConstants.Launcher.QueryPrefix.matches(query, prefixName: "d") {
            return .folders
        }
        return .unscoped
    }

    static func normalizedPinnedLookupQuery(
        for query: String,
        scope: LauncherPinnedLookupScope
    ) -> String? {
        var normalized: String?
        switch scope {
        case .disabled:
            return nil
        case .apps:
            normalized = AppConstants.Launcher.QueryPrefix.strip(from: query, prefixName: "a")
        case .files:
            normalized = AppConstants.Launcher.QueryPrefix.strip(from: query, prefixName: "f")
        case .folders:
            normalized = AppConstants.Launcher.QueryPrefix.strip(from: query, prefixName: "d")
        case .unscoped:
            normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        }

        guard let norm = normalized, !norm.isEmpty else {
            return nil
        }

        return norm.lowercased()
    }

    static func shouldInjectFinder(
        normalizedQuery: String?,
        scope: LauncherPinnedLookupScope
    ) -> Bool {
        guard scope == .unscoped || scope == .apps else { return false }
        guard let normalized = normalizedQuery else { return false }

        let finderName = AppConstants.Launcher.Finder.appName
        return normalized.contains(finderName)
            || (finderName.hasPrefix(normalized)
                && normalized.count >= AppConstants.Launcher.Finder.minPrefixMatchLength)
    }

    static func dedupe(results: [LauncherResult]) -> [LauncherResult] {
        var seen = Set<String>()
        var unique: [LauncherResult] = []

        for item in results {
            let key = dedupeKey(for: item)
            if key.isEmpty || seen.insert(key).inserted {
                unique.append(item)
            }
        }

        return unique
    }

    private static func dedupeKey(for result: LauncherResult) -> String {
        let normalizedTitle = result.title.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let normalizedPath = result.path.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

        switch result.kind {
        case .app:
            return "\(result.kind.rawValue):\(normalizedTitle)"
        case .file, .folder:
            return "\(result.kind.rawValue):\(normalizedPath)"
        case .clipboard, .process, .action:
            // Clipboard entries, process rows, and declared blocks are already
            // unique by id (entry UUID / pid / block id), so key on it rather
            // than title/path.
            return result.id
        }
    }
}
