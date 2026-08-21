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
        quickActionDescriptors
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
            performSourceBlockTarget(blockID: pending.blockID, title: pending.title)
            return
        }

        // Asking one. The menu stays open and swaps to the question, so the
        // answer happens where the user's eyes already are.
        if let blockID = SourceBlockAction.blockID(fromActionID: descriptor.actionId),
           let question = descriptor.confirm {
            pendingActionConfirm = (blockID: blockID, title: descriptor.title, question: question)
            // Start on Cancel: a destructive action should cost one more press
            // than an accidental double-Enter.
            actionMenuIndex = 1
            return
        }

        closeActionMenu()
        runQuickAction(descriptor, intent: descriptor.control == .toggle ? .toggle : .run)
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
