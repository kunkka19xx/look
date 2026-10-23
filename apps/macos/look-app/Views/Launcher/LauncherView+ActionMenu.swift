import SwiftUI

extension LauncherView {
    /// Actions offered for the selected row: its compiled controls plus any
    /// `then` targets its block declared.
    ///
    /// The launchpad deliberately has none. Its tiles are already the actions,
    /// each with its own mnemonic (⌘B, ⌘W, ⌘T...), so a menu there would list
    /// what is on screen and cover it - and ⌘K is Keep Awake, so opening one
    /// would shadow the key the user meant.
    var actionMenuDescriptors: [QuickActionDescriptor] {
        compactPanelDescriptors + quickActionDescriptors
    }

    /// Ids of the two rows the menu shows in place of the action list while a
    /// destructive target waits for an answer.
    enum Confirm {
        static let yes = "srcconfirm:yes"
        static let no = "srcconfirm:no"
        static let cancelTitle = "Cancel"
    }

    /// What the menu lists right now: the pending question when one is waiting,
    /// otherwise the row's actions.
    ///
    /// The confirmation reuses the menu rather than opening a modal, so the
    /// keys the user already has (move, Enter, Escape) keep working and Escape
    /// means the safe thing.
    var actionMenuRows: [QuickActionDescriptor] {
        guard let pending = pendingActionConfirm else { return actionMenuDescriptors }
        return [
            QuickActionDescriptor(
                actionId: Confirm.yes, title: pending.question, control: .button,
                onLabel: nil, offLabel: nil, info: []),
            QuickActionDescriptor(
                actionId: Confirm.no, title: Confirm.cancelTitle, control: .button,
                onLabel: nil, offLabel: nil, info: []),
        ]
    }

    /// Cmd+K. Opening with nothing to offer would show an empty box, so a row
    /// with no actions says so instead.
    func toggleActionMenu() {
        if isActionMenuOpen {
            closeActionMenu()
            return
        }
        catchUpPanel()
        guard !actionMenuDescriptors.isEmpty else {
            showBanner("Nothing to do here", style: .info, duration: 1.2)
            return
        }
        actionMenuIndex = 0
        withAnimation(Motion.Selection.glide) { isActionMenuOpen = true }
    }

    func closeActionMenu() {
        pendingActionConfirm = nil
        guard isActionMenuOpen else { return }
        withAnimation(Motion.Selection.glide) { isActionMenuOpen = false }
        actionMenuIndex = 0
    }

    /// Wraps at both ends, so holding one direction cycles rather than dead-ends.
    func moveActionMenuFocus(by offset: Int) {
        let count = actionMenuRows.count
        guard count > 0 else { return }
        actionMenuIndex = ((actionMenuIndex + offset) % count + count) % count
    }

    func runFocusedAction() {
        let rows = actionMenuRows
        guard rows.indices.contains(actionMenuIndex) else { return }
        activateActionMenuRow(rows[actionMenuIndex])
    }

    /// Activating one row of the menu, by key or by click.
    ///
    /// The single entry point on purpose: a click that bypassed this would skip
    /// the confirmation, so a `confirm` target would ask when reached with Enter
    /// and delete silently when reached with the mouse.
    func activateActionMenuRow(_ descriptor: QuickActionDescriptor) {
        // Answering a pending question.
        if let pending = pendingActionConfirm {
            pendingActionConfirm = nil
            closeActionMenu()
            guard descriptor.actionId == Confirm.yes else { return }
            guard let plan = pending.plan else {
                performSourceBlockTarget(blockID: pending.blockID, title: pending.title)
                return
            }
            runPlannedVerb(plan: plan, title: pending.title)
            return
        }

        // Asking one. The menu stays open and swaps to the question, so the
        // answer happens where the user's eyes already are.
        if let blockID = SourceBlockAction.blockID(fromActionID: descriptor.actionId),
           plansItsOwnQuestion(blockID) {
            askTheConnector(blockID: blockID, title: descriptor.title)
            return
        }

        if let blockID = SourceBlockAction.blockID(fromActionID: descriptor.actionId),
           let question = descriptor.confirm {
            pendingActionConfirm = (
                blockID: blockID, title: descriptor.title, question: question, plan: nil
            )
            // Start on Cancel: a destructive action should cost one more press
            // than an accidental double-Enter.
            actionMenuIndex = 1
            return
        }

        if RowAction.isOne(descriptor.actionId) {
            closeActionMenu()
            activateRowAction(descriptor.actionId)
            return
        }

        closeActionMenu()
        runQuickAction(descriptor, intent: descriptor.control == .toggle ? .toggle : .run)
    }

    /// Whether this target is a verb: the connector that owns the row works
    /// out what it would do, so the question is computed rather than declared.
    private func plansItsOwnQuestion(_ blockID: String) -> Bool {
        guard let selected = actionableSelectedResult() else { return false }
        return SourceBlockCatalog.targets(for: selected)
            .own
            .first { $0.id == blockID }?
            .plans ?? false
    }

    /// Asks the connector what the verb would do, then shows that as the
    /// question. The menu stays open while it waits, since the answer belongs
    /// where the user's eyes already are.
    private func askTheConnector(blockID: String, title: String) {
        guard let selected = actionableSelectedResult() else { return }
        let row = RowRef(selected)
        let candidate = selected.id
        let typed = query
        let ancestors = selectedRowAncestorsJSON

        Task {
            let planned = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.planVerb(
                    blockID: blockID, candidateID: candidate, row: row, query: typed,
                    ancestorsJSON: ancestors)
            }.value

            await MainActor.run {
                // The menu closed, or moved on and came back, while the
                // connector was thinking. Either way this answer is stale, and
                // setting it would put an old question on a new row.
                guard isActionMenuOpen, actionableSelectedResult()?.id == candidate
                else { return }
                guard let planned, planned.error == nil else {
                    closeActionMenu()
                    showBanner(
                        "\(title): \(planned?.error ?? "the core did not answer")",
                        style: .error, duration: 4.0)
                    return
                }
                let question = [planned.title, planned.detail]
                    .compactMap { $0 }
                    .filter { !$0.isEmpty }
                    .joined(separator: ": ")
                pendingActionConfirm = (
                    blockID: blockID, title: title, question: question, plan: planned.plan
                )
                // Starts on Yes: the question names what will happen and the
                // verb was picked on purpose. The declared `confirm` string
                // below starts on Cancel, where the text cannot say as much.
                actionMenuIndex = 0
            }
        }
    }

    /// Runs what the user just confirmed, and says how it went.
    private func runPlannedVerb(plan: UInt64, title: String) {
        guard let selected = actionableSelectedResult() else { return }
        let candidate = selected.id
        Task {
            let ran = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.runVerb(candidateID: candidate, plan: plan)
            }.value
            await MainActor.run {
                if let failure = ran?.error ?? (ran == nil ? "the core did not answer" : nil) {
                    showBanner("\(title): \(failure)", style: .error, duration: 4.0)
                    return
                }
                guard let ran else { return }
                // No summary means the connector has not answered yet, which
                // is not the same as it having finished.
                showBanner(
                    ran.summary ?? "\(title): started", style: ran.ok ? .info : .warning,
                    duration: ran.ok ? 2.0 : 4.0)
            }
        }
    }

    /// Keeps the focused row inside the list when the offered actions change
    /// under it (a launchpad tile resolving, a block reloading).
    func clampActionMenuFocus() {
        let count = actionMenuRows.count
        guard count > 0 else {
            closeActionMenu()
            return
        }
        actionMenuIndex = min(actionMenuIndex, count - 1)
    }
}
