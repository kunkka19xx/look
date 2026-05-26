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
        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

        if normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.regex)
            || normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.clipboard)
        {
            return .disabled
        }
        if normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.apps) {
            return .apps
        }
        if normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.files) {
            return .files
        }
        if normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.folders) {
            return .folders
        }
        return .unscoped
    }

    static func normalizedPinnedLookupQuery(
        for query: String,
        scope: LauncherPinnedLookupScope
    ) -> String? {
        var normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

        if scope == .apps, normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.apps) {
            normalized = String(normalized.dropFirst(AppConstants.Launcher.QueryPrefix.apps.count))
                .trimmingCharacters(in: .whitespacesAndNewlines)
        } else if scope == .files, normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.files) {
            normalized = String(normalized.dropFirst(AppConstants.Launcher.QueryPrefix.files.count))
                .trimmingCharacters(in: .whitespacesAndNewlines)
        } else if scope == .folders, normalized.hasPrefix(AppConstants.Launcher.QueryPrefix.folders) {
            normalized = String(normalized.dropFirst(AppConstants.Launcher.QueryPrefix.folders.count))
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }

        if scope == .disabled || normalized.isEmpty {
            return nil
        }

        return normalized
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
        case .clipboard:
            return result.id
        }
    }
}
