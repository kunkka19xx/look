import SwiftUI

extension LauncherView {
    /// Actions offered right now: for a selected row, its compiled controls plus
    /// any `then` targets its block declared. With an empty query there is no
    /// row, but the launchpad is on screen and its tiles are actions too, so the
    /// menu offers those instead of refusing.
    var actionMenuDescriptors: [QuickActionDescriptor] {
        if selectedResultID != nil, !quickActionDescriptors.isEmpty {
            return quickActionDescriptors
        }
        return isLaunchpadActive ? launchpadActionDescriptors : quickActionDescriptors
    }

    /// The launchpad's own controls, as menu rows. Display-only tiles (weather,
    /// battery, now playing, the L slot) are things to read, not to do, so they
    /// stay out of a list of verbs.
    private var launchpadActionDescriptors: [QuickActionDescriptor] {
        launchpadTiles
            .filter { $0.role == .toggle || $0.role == .action }
            .map { tile in
                QuickActionDescriptor(
                    actionId: tile.actionId,
                    title: tile.title,
                    control: tile.role == .toggle ? .toggle : .button,
                    onLabel: tile.onLabel,
                    offLabel: tile.offLabel,
                    info: []
                )
            }
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
        guard isActionMenuOpen else { return }
        withAnimation(Motion.Selection.glide) { isActionMenuOpen = false }
        actionMenuIndex = 0
    }

    /// Wraps at both ends, so holding one direction cycles rather than dead-ends.
    func moveActionMenuFocus(by offset: Int) {
        let count = actionMenuDescriptors.count
        guard count > 0 else { return }
        actionMenuIndex = ((actionMenuIndex + offset) % count + count) % count
    }

    func runFocusedAction() {
        let descriptors = actionMenuDescriptors
        guard descriptors.indices.contains(actionMenuIndex) else { return }
        let descriptor = descriptors[actionMenuIndex]
        closeActionMenu()
        runQuickAction(descriptor, intent: descriptor.control == .toggle ? .toggle : .run)
    }

    /// Keeps the focused row inside the list when the offered actions change
    /// under it (a launchpad tile resolving, a block reloading).
    func clampActionMenuFocus() {
        let count = actionMenuDescriptors.count
        guard count > 0 else {
            closeActionMenu()
            return
        }
        actionMenuIndex = min(actionMenuIndex, count - 1)
    }
}
