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
    private static var targetsByBlockID: [String: [SourceBlockTarget]] = [:]

    static func invalidate() {
        iconsByBlockID = nil
        targetsByBlockID = [:]
    }

    /// The `then` targets of the block a candidate id belongs to.
    ///
    /// Memoised per block: this is read while building the panel on every
    /// selection change, and arrow-keying down a list of projects should not
    /// re-read the sources directory once per row. `invalidate()` on config
    /// reload is what picks up an edited `then`.
    static func targets(forCandidateID candidateID: String) -> [SourceBlockTarget] {
        guard let blockID = blockID(fromCandidateID: candidateID) else { return [] }
        if let cached = targetsByBlockID[blockID] { return cached }

        let targets = EngineBridge.shared.sourceBlock(candidateID: candidateID)?.then ?? []
        targetsByBlockID[blockID] = targets
        return targets
    }

    /// The declared icon for the block a candidate id belongs to.
    static func icon(forCandidateID candidateID: String) -> String? {
        guard let blockID = blockID(fromCandidateID: candidateID) else { return nil }
        if iconsByBlockID == nil {
            iconsByBlockID = Dictionary(
                EngineBridge.shared.sourceBlocks().compactMap { summary in
                    summary.icon.map { (summary.id, $0) }
                },
                uniquingKeysWith: { first, _ in first }
            )
        }
        return iconsByBlockID?[blockID]
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

    /// Where `open -a Name` would find the bundle. The same roots `open` itself
    /// searches, plus a bundle-id lookup for a step that names one.
    private static func bundlePath(forAppNamed name: String) -> String? {
        let roots = [
            "/Applications",
            "/System/Applications",
            "/System/Applications/Utilities",
            "/Applications/Utilities",
            NSHomeDirectory() + "/Applications",
        ]
        let bundleName = name.hasSuffix(".app") ? name : name + ".app"
        for root in roots {
            let candidate = root + "/" + bundleName
            if FileManager.default.fileExists(atPath: candidate) {
                return candidate
            }
        }
        return NSWorkspace.shared
            .urlForApplication(withBundleIdentifier: name)?
            .path
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
