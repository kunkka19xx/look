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
    /// The `then` targets of the block behind `result`, as panel descriptors.
    ///
    /// Read synchronously, from a per-block memo: the panel is built during a
    /// selection change, and an async load that appends later would make the
    /// action list grow under the user's cursor.
    func sourceBlockTargets(for result: LauncherResult) -> [QuickActionDescriptor] {
        guard result.id.hasPrefix(AppConstants.Launcher.SourceBlock.idPrefix) else { return [] }

        return SourceBlockCatalog.targets(forCandidateID: result.id).map { target in
            QuickActionDescriptor(
                actionId: SourceBlockAction.actionID(forBlockID: target.id),
                title: target.performs ? target.name : "\(target.name)…",
                control: .button,
                onLabel: nil,
                offLabel: nil,
                info: []
            )
        }
    }

    /// Runs a `then` target against the selected row, which is what its
    /// placeholders expand to.
    func performSourceBlockTarget(blockID: String, title: String) {
        guard let selected = actionableSelectedResult() else { return }

        let row = (id: selected.id, title: selected.title, path: selected.path, query: query)
        Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.performBlock(
                    blockID: blockID,
                    rowID: row.id,
                    rowTitle: row.title,
                    rowPath: row.path,
                    query: row.query
                )
            }.value

            await MainActor.run {
                if let failure = outcome.errors.first {
                    showBanner("\(title): \(failure)", style: .error, duration: 4.0)
                    return
                }
                if outcome.performed == 0 {
                    // A target that produces rows rather than performing steps.
                    // Descending into it needs the level stack, which does not
                    // exist yet, so say so rather than appearing to do nothing.
                    showBanner("\(title) produces rows; drill-down is not built yet", style: .info, duration: 2.4)
                    return
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        }
    }
}
