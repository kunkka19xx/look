import AppKit
import SwiftUI

/// A `then` target rendered as a quick action.
///
/// Compiled controls key on a build-time `action_id`; these key on the block
/// they run. The prefix is what tells the two apart at dispatch, so a declared
/// target can never collide with a control id.
enum SourceBlockAction {
    private static let actionIDPrefix = "srcblock:"

    static func actionID(forBlockID blockID: String) -> String {
        actionIDPrefix + blockID
    }

    static func blockID(fromActionID actionID: String) -> String? {
        guard actionID.hasPrefix(actionIDPrefix) else { return nil }
        return String(actionID.dropFirst(actionIDPrefix.count))
    }
}

extension LauncherView {
    /// Declared targets as panel entries. The ellipsis says one lists rather
    /// than runs: it opens a level.
    func sourceBlockDescriptors(_ targets: [SourceBlockTarget]) -> [QuickActionDescriptor] {
        targets.map { target in
            QuickActionDescriptor(
                actionId: SourceBlockAction.actionID(forBlockID: target.id),
                title: target.performs ? target.name : "\(target.name)…",
                control: .button,
                onLabel: nil,
                offLabel: nil,
                info: [],
                confirm: target.confirm
            )
        }
    }

    /// Runs a `then` target against the selected row, which is what its
    /// placeholders expand to.
    func performSourceBlockTarget(blockID: String, title: String) {
        guard let selected = actionableSelectedResult() else { return }

        let row = RowRef(selected)
        let typed = query
        let ancestors = selectedRowAncestorsJSON
        // Claimed before the block runs: the user can hide the launcher or
        // start another target while it does.
        let epoch = levelStack.beginRequest()
        let parent = LevelParentRow(
            candidateID: selected.id,
            title: selected.title,
            path: selected.path,
            openedFromQuery: query,
            openedFromSelection: selectedResultID
        )
        Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.performBlock(
                    blockID: blockID, row: row, query: typed, ancestorsJSON: ancestors,
                    asTarget: true)
            }.value

            await MainActor.run {
                guard epoch == levelStack.epoch else { return }
                if let failure = outcome.errors.first {
                    showBanner("\(title): \(failure)", style: .error, duration: 4.0)
                    return
                }
                if outcome.producesRows {
                    // Not a failure and nothing was performed: the target lists,
                    // so it is a level to descend into.
                    descendIntoBlock(
                        blockID: blockID, title: title, parent: parent, ancestorsJSON: ancestors,
                        epoch: epoch)
                    return
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        }
    }
}
