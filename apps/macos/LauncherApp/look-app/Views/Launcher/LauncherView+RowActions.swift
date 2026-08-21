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

    /// Which tool each action would start, so a label can name it instead of
    /// saying "Edit" and hoping. Cheap after the first call: the tools come from
    /// the process-wide config cache and resolving is string work.
    private func resolvedTools(for result: LauncherResult) -> [String: ToolAction] {
        var resolved: [String: ToolAction] = [:]
        for action in [
            AppConstants.Launcher.Tools.editAction,
            AppConstants.Launcher.Tools.terminalAction,
            AppConstants.Launcher.Tools.revealAction,
        ] where Self.toolActionApplies(action, to: result.kind) {
            if let outcome = EngineBridge.shared.toolAction(
                action, path: result.path, isDirectory: result.kind == .folder
            ) {
                resolved[action] = outcome
            }
        }
        return resolved
    }

    func rowActionDescriptors(for result: LauncherResult) -> [QuickActionDescriptor] {
        guard !result.path.isEmpty, result.kind != .clipboard, result.kind != .process else {
            return []
        }

        let tools = resolvedTools(for: result)
        let applies = { Self.toolActionApplies($0, to: result.kind) }
        var entries: [(id: String, title: String)] = [(RowAction.open, "Open  ⏎")]

        if applies(AppConstants.Launcher.Tools.editAction) {
            entries.append((
                RowAction.edit,
                "\(title("Edit", "Edit in", tools, AppConstants.Launcher.Tools.editAction))  ⌘E"
            ))
        }
        if applies(AppConstants.Launcher.Tools.terminalAction) {
            entries.append((
                RowAction.terminal,
                "\(title("Open terminal here", "Open in", tools, AppConstants.Launcher.Tools.terminalAction))  ⌘T"
            ))
        }
        if applies(AppConstants.Launcher.Tools.revealAction) {
            entries.append((
                RowAction.reveal,
                "Reveal in \(tools[AppConstants.Launcher.Tools.revealAction]?.tool ?? AppConstants.Launcher.Tools.systemFileManagerName)  ⌘F"
            ))
        }
        entries.append((RowAction.copyPath, "Copy path  ⌘C"))

        return entries.map { entry in
            QuickActionDescriptor(
                actionId: entry.id,
                title: entry.title,
                control: .button,
                onLabel: nil,
                offLabel: nil,
                info: []
            )
        }
    }

    /// `<named> <tool>` when a tool resolved, the plain wording otherwise.
    private func title(
        _ plain: String, _ named: String, _ tools: [String: ToolAction], _ action: String
    ) -> String {
        guard let tool = tools[action]?.tool else { return plain }
        return "\(named) \(tool)"
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
