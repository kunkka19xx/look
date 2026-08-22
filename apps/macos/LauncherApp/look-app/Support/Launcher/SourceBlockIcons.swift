import AppKit

/// What each declared block asked to be drawn as, keyed by block id.
///
/// Rows render synchronously and there are many of them, so this is read on
/// first use and kept for the process rather than fetched per row. The files
/// involved are a handful of small TOMLs, and `invalidate()` drops the cache
/// when they change.
@MainActor
enum SourceBlockCatalog {
    private static var iconsByBlockID: [String: String]?
    private static var targetsByCandidateID: [String: [SourceBlockTarget]] = [:]
    private static var targetsInFlight: Set<String> = []

    static func invalidate() {
        iconsByBlockID = nil
        targetsByCandidateID = [:]
        targetsInFlight = []
        prefill()
    }

    /// The `then` targets for one row.
    ///
    /// Keyed on the candidate rather than the block, because a target's confirm
    /// question is expanded against the row ("Delete main?"). Memoised because
    /// this is read while building the panel on every selection change, and
    /// arrow-keying down a list should not re-read the sources directory once
    /// per row. `invalidate()` on config reload picks up an edited `then`.
    static func targets(for result: LauncherResult) -> [SourceBlockTarget] {
        if let cached = targetsByCandidateID[cacheKey(for: result)] { return cached }
        loadTargets(for: result)
        return []
    }

    /// Reads one row's targets off the main actor and caches them. The panel
    /// re-reads on the next selection change, so an empty first answer costs a
    /// moment rather than an action.
    private static func loadTargets(for result: LauncherResult) {
        let key = cacheKey(for: result)
        guard !targetsInFlight.contains(key) else { return }
        targetsInFlight.insert(key)

        Task {
            let targets = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.sourceBlock(
                    candidateID: result.id,
                    rowID: result.id,
                    rowTitle: result.title,
                    rowPath: result.path
                )?.then ?? []
            }.value
            await MainActor.run {
                targetsByCandidateID[key] = targets
                targetsInFlight.remove(key)
                NotificationCenter.default.post(name: .lookSourceTargetsLoaded, object: nil)
            }
        }
    }

    /// A target's `confirm` text is expanded against the row, so two rows that
    /// share an id but differ in title or path must not share a cache entry.
    private static func cacheKey(for result: LauncherResult) -> String {
        "\(result.id)\u{1}\(result.title)\u{1}\(result.path)"
    }

    /// The declared icon for the block a candidate id belongs to.
    ///
    /// Never reads from disk: rows render synchronously and there are many of
    /// them, so a cache miss returns nil and the row falls back to the generic
    /// glyph until `prefill()` lands. A momentarily generic icon is a far
    /// smaller cost than a directory walk on the main actor mid-render.
    static func icon(forCandidateID candidateID: String) -> String? {
        guard let blockID = blockID(fromCandidateID: candidateID) else { return nil }
        return iconsByBlockID?[blockID]
    }

    /// Loads the block catalog off the main actor. Called on launcher open and
    /// after a config reload, so the caches are warm before anything renders.
    static func prefill() {
        Task {
            let summaries = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.sourceBlocks()
            }.value
            await MainActor.run {
                iconsByBlockID = Dictionary(
                    summaries.compactMap { summary in
                        summary.icon.map { (summary.id, $0) }
                    },
                    uniquingKeysWith: { first, _ in first }
                )
            }
        }
    }

    /// `src:<block>:<row>` -> `<block>`.
    static func blockID(fromCandidateID candidateID: String) -> String? {
        let prefix = AppConstants.Launcher.SourceBlock.idPrefix
        guard candidateID.hasPrefix(prefix) else { return nil }
        let rest = candidateID.dropFirst(prefix.count)
        guard let separator = rest.firstIndex(of: ":") else { return nil }
        return String(rest[rest.startIndex..<separator])
    }
}

/// Icons for rows and panels that come from a user-declared block.
///
/// Two sources, in order of trust: what the block declared (`icon = …`), then
/// what its steps obviously do. Guessing is deliberately narrow, since a wrong
/// icon is worse than a generic one.
enum SourceBlockIcons {
    /// Row-icon size. Larger than the list needs, so the same cached image can
    /// serve the panel without looking soft.
    private static let renderedSize = NSSize(width: 64, height: 64)

    /// `open -a Slack`, `open -a "Google Chrome" https://…`. Anything else (a
    /// CLI like `code`, a shell pipeline) resolves to nothing rather than a
    /// guess.
    static func appName(inStep step: String) -> String? {
        let trimmed = step.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("open ") else { return nil }

        var rest = Substring(trimmed.dropFirst("open ".count))
        while let flagRange = rest.range(of: "-a ") {
            rest = rest[flagRange.upperBound...]
            return firstArgument(in: rest)
        }
        return nil
    }

    /// The declared `icon`, resolved as an image path, an SF Symbol name, or
    /// text to draw (an emoji). Nil when the block declared nothing.
    static func declaredIcon(_ declared: String?) -> NSImage? {
        guard let declared, !declared.trimmingCharacters(in: .whitespaces).isEmpty else {
            return nil
        }
        return RowIconCache.image(key: "blockicon:\(declared)") {
            if FileManager.default.fileExists(atPath: declared),
               let image = NSImage(contentsOfFile: declared) {
                return image
            }
            if let symbol = NSImage(systemSymbolName: declared, accessibilityDescription: nil) {
                return symbol
            }
            return rendered(text: declared)
        }
    }

    /// The app icons a bundle's steps will bring up, deduplicated and in step
    /// order. Steps that name no app contribute nothing, so the strip never
    /// implies more than the block does.
    static func appIcons(forSteps steps: [String], limit: Int) -> [NSImage] {
        var seen = Set<String>()
        var icons: [NSImage] = []
        for step in steps {
            guard icons.count < limit,
                  let name = appName(inStep: step),
                  seen.insert(name.lowercased()).inserted,
                  let path = bundlePath(forAppNamed: name)
            else { continue }
            icons.append(RowIconCache.icon(forFile: path))
        }
        return icons
    }

    private static func bundlePath(forAppNamed name: String) -> String? {
        AppBundleLocator.bundlePath(forAppNamed: name)
    }

    /// The first argument after `-a`, honouring quotes so a two-word app name
    /// survives.
    private static func firstArgument(in text: Substring) -> String? {
        let trimmed = text.drop(while: { $0 == " " })
        guard let first = trimmed.first else { return nil }

        if first == "\"" || first == "'" {
            let body = trimmed.dropFirst()
            guard let end = body.firstIndex(of: first) else { return nil }
            let name = String(body[body.startIndex..<end])
            return name.isEmpty ? nil : name
        }

        let name = String(trimmed.prefix(while: { $0 != " " }))
        return name.isEmpty ? nil : name
    }

    /// Draws text (an emoji) into an image, so a declared glyph can sit in the
    /// same image slot as an app icon.
    private static func rendered(text: String) -> NSImage {
        let image = NSImage(size: renderedSize)
        image.lockFocus()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: renderedSize.height * 0.72)
        ]
        let string = text as NSString
        let bounds = string.size(withAttributes: attributes)
        string.draw(
            at: NSPoint(
                x: (renderedSize.width - bounds.width) / 2,
                y: (renderedSize.height - bounds.height) / 2
            ),
            withAttributes: attributes
        )
        image.unlockFocus()
        return image
    }
}
