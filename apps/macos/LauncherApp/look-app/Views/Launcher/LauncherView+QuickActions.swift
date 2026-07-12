import Foundation

/// Quick Actions - data loading and execution for the info+actions panel (see
/// docs/writing-controls.md). Descriptors come from the shared `look_qactions`
/// catalog; each action's live state and its execution come from a native
/// `SystemControl` adapter resolved by `actionId`.
///
/// Interaction: a `.toggle` control is flipped with Cmd+O (no navigation).
/// Multi-choice controls (future) will move between options with Cmd+J/K.
extension LauncherView {
    /// Banner durations (seconds) for action outcomes.
    private enum Banner {
        static let success: TimeInterval = 1.2
        static let error: TimeInterval = 1.6
        static let needsPermission: TimeInterval = 2.2
        static let unavailable: TimeInterval = 1.4
    }

    /// The selected result, if it is a real candidate (not a synthesized row).
    private var selectedResultForActions: LauncherResult? {
        guard let id = selectedResultID else { return nil }
        return displayedResults.first(where: { $0.id == id })
    }

    /// The selected result's primary toggle action, if any (drives Cmd+O).
    var toggleQuickAction: QuickActionDescriptor? {
        quickActionDescriptors.first(where: { $0.control == .toggle })
    }

    /// Whether Cmd+O has a toggle to act on for the current selection.
    var hasToggleQuickAction: Bool { toggleQuickAction != nil }

    /// Loads the selected result's Quick Actions and reads each one's live state
    /// off the main thread. Cancels any in-flight read so a stale result never
    /// populates the panel. Called on selection/query change.
    func refreshQuickActions() {
        quickActionTask?.cancel()

        guard let result = selectedResultForActions else {
            if !quickActionDescriptors.isEmpty { quickActionDescriptors = [] }
            if !quickActionStates.isEmpty { quickActionStates = [:] }
            return
        }

        let descriptors = bridge.quickActions(forResultID: result.id, kind: result.kind.rawValue)
        quickActionDescriptors = descriptors
        quickActionStates = [:]
        guard !descriptors.isEmpty else { return }

        let resultID = result.id
        quickActionTask = Task {
            for descriptor in descriptors {
                guard !Task.isCancelled else { return }
                let state: ActionState
                if let adapter = ActionAdapterRegistry.adapter(for: descriptor.actionId) {
                    state = await adapter.state()
                } else {
                    state = .unavailable("Not supported on this Mac")
                }
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    // Drop the read if the selection moved on while we awaited.
                    guard selectedResultID == resultID else { return }
                    quickActionStates[descriptor.actionId] = state
                }
            }
        }
    }

    /// Flips the selected result's primary toggle (Cmd+O).
    func togglePrimaryQuickAction() {
        guard let descriptor = toggleQuickAction else { return }
        runQuickAction(descriptor, intent: .toggle)
    }

    /// Runs a specific action's intent (from a click or a key), shows the
    /// outcome, and refreshes its state. Shared by the toggle switch and Cmd+O.
    func runQuickAction(_ descriptor: QuickActionDescriptor, intent: ActionIntent) {
        guard let adapter = ActionAdapterRegistry.adapter(for: descriptor.actionId) else {
            showBanner("\(descriptor.title) is not available", style: .info, duration: Banner.unavailable)
            return
        }

        // Flip a toggle immediately for instant feedback; the re-read below
        // confirms (and corrects it if the change did not take).
        if intent == .toggle {
            switch quickActionStates[descriptor.actionId] {
            case .on?: quickActionStates[descriptor.actionId] = .off
            case .off?: quickActionStates[descriptor.actionId] = .on
            default: break
            }
        }

        Task {
            let outcome = await adapter.apply(intent)
            await MainActor.run {
                switch outcome {
                case .ok(let banner):
                    showBanner(banner ?? "\(descriptor.title) done", style: .success, duration: Banner.success)
                case .failed(let message):
                    showBanner(message, style: .error, duration: Banner.error)
                case .needsPermission(let message):
                    showBanner(message, style: .info, duration: Banner.needsPermission)
                }
            }
            let state = await adapter.state()
            await MainActor.run {
                quickActionStates[descriptor.actionId] = state
            }
        }
    }
}
