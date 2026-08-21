import Foundation

/// The Cmd+K entries every row with a path offers. Most already worked as chords
/// nobody could discover; the panel is where they become visible.
///
/// Suppressed for a row whose block declared `then` targets: the author already
/// chose that row's vocabulary and every entry here keeps its chord anyway
/// (`specs/preferred-tools.md` §7.1).
extension LauncherView {
    enum RowAction {
        static let prefix = "rowaction:"

        static let open = "\(prefix)open"
        static let edit = "\(prefix)edit"
        static let terminal = "\(prefix)terminal"
        static let reveal = "\(prefix)reveal"
        static let copyPath = "\(prefix)copypath"

        static func isOne(_ actionID: String) -> Bool {
            actionID.hasPrefix(prefix)
        }
    }

    /// One panel entry. `named` is the wording used once a tool resolves, so
    /// "Edit" becomes "Edit in Zed" and the menu teaches what the chord opens.
    private struct RowActionEntry {
        let id: String
        let plain: String
        let named: String?
        let chord: String
        /// The tool action whose resolved name fills `named`, when there is one.
        let tool: String?
        /// Used in place of a declared tool, for an action the platform always
        /// has an answer for.
        let fallbackTool: String?

        init(
            id: String, plain: String, named: String? = nil, chord: String,
            tool: String? = nil, fallbackTool: String? = nil
        ) {
            self.id = id
            self.plain = plain
            self.named = named
            self.chord = chord
            self.tool = tool
            self.fallbackTool = fallbackTool
        }
    }

    /// Separates a label from the chord that already performs it.
    private static let chordGap = "  "

    private static var rowActionCatalog: [RowActionEntry] {
        [
            RowActionEntry(id: RowAction.open, plain: "Open", chord: "⏎"),
            RowActionEntry(
                id: RowAction.edit, plain: "Edit", named: "Edit in", chord: "⌘E",
                tool: AppConstants.Launcher.Tools.editAction),
            RowActionEntry(
                id: RowAction.terminal, plain: "Open terminal here", named: "Open in",
                chord: "⌘T", tool: AppConstants.Launcher.Tools.terminalAction),
            RowActionEntry(
                id: RowAction.reveal, plain: "Reveal", named: "Reveal in", chord: "⌘F",
                tool: AppConstants.Launcher.Tools.revealAction,
                fallbackTool: AppConstants.Launcher.Tools.systemFileManagerName),
            RowActionEntry(id: RowAction.copyPath, plain: "Copy path", chord: "⌘C"),
        ]
    }

    func rowActionDescriptors(for result: LauncherResult) -> [QuickActionDescriptor] {
        guard !result.path.isEmpty, result.kind != .clipboard, result.kind != .process else {
            return []
        }

        let offered = Self.rowActionCatalog.filter { entry in
            guard let action = entry.tool else { return true }
            return Self.toolActionApplies(action, to: result.kind)
        }
        let tools = resolvedTools(for: result, entries: offered)

        return offered.map { entry in
            QuickActionDescriptor(
                actionId: entry.id,
                title: Self.label(for: entry, tools: tools),
                control: .button,
                onLabel: nil,
                offLabel: nil,
                info: []
            )
        }
    }

    private static func label(for entry: RowActionEntry, tools: [String: ToolAction]) -> String {
        let resolved = entry.tool.flatMap { tools[$0]?.tool } ?? entry.fallbackTool
        let wording =
            if let named = entry.named, let resolved { "\(named) \(resolved)" } else { entry.plain }
        return "\(wording)\(chordGap)\(entry.chord)"
    }

    /// Which tool each offered action would start. Cheap after the first call:
    /// the tools come from the process-wide config cache and resolving is string
    /// work.
    private func resolvedTools(
        for result: LauncherResult, entries: [RowActionEntry]
    ) -> [String: ToolAction] {
        var resolved: [String: ToolAction] = [:]
        for action in entries.compactMap(\.tool) {
            if let outcome = EngineBridge.shared.toolAction(
                action, path: result.path, isDirectory: result.kind == .folder
            ) {
                resolved[action] = outcome
            }
        }
        return resolved
    }

    func activateRowAction(_ actionID: String) {
        switch actionID {
        case RowAction.open:
            openSelectedApp()
        case RowAction.edit:
            editSelectedResult()
        case RowAction.terminal:
            openTerminalForSelectedResult()
        case RowAction.reveal:
            revealSelectedResult()
        case RowAction.copyPath:
            _ = copySelectedResultToPasteboard()
        default:
            break
        }
    }
}
