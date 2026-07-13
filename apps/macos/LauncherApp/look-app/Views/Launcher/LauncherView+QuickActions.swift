import Foundation

/// Quick Actions - data loading and execution for the info+actions panel (see
/// docs/writing-controls.md). Descriptors come from the shared `look_qactions`
/// catalog; each action's live state, info (e.g. paired devices), and execution
/// come from a native `SystemControl` adapter resolved by `actionId`.
///
/// Interaction: a `.toggle` control is flipped with Cmd+O; a list item (e.g. a
/// paired device) is connected/disconnected by clicking its row.
extension LauncherView {
    /// Banner durations (seconds) for action outcomes.
    private enum Banner {
        static let success: TimeInterval = 1.2
        static let error: TimeInterval = 1.6
        static let needsPermission: TimeInterval = 2.2
        static let unavailable: TimeInterval = 1.4
        /// "Connecting to…" stays until the outcome replaces it; long enough to
        /// outlast a device connect that times out (deviceActionTimeout + buffer).
        static let inProgress: TimeInterval = 8
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
    /// and info off the main thread. Cancels any in-flight read so a stale result
    /// never populates the panel. Called on selection/query change.
    func refreshQuickActions() {
        quickActionTask?.cancel()

        guard let result = selectedResultForActions else {
            if !quickActionDescriptors.isEmpty { quickActionDescriptors = [] }
            if !quickActionStates.isEmpty { quickActionStates = [:] }
            if !quickActionInfo.isEmpty { quickActionInfo = [:] }
            return
        }

        let descriptors = bridge.quickActions(forResultID: result.id, kind: result.kind.rawValue)
        quickActionDescriptors = descriptors
        quickActionStates = [:]
        quickActionInfo = [:]
        guard !descriptors.isEmpty else { return }

        let resultID = result.id
        quickActionTask = Task {
            for descriptor in descriptors {
                guard !Task.isCancelled else { return }
                let (state, info) = await readQuickAction(descriptor)
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    // Drop the read if the selection moved on while we awaited.
                    guard selectedResultID == resultID else { return }
                    apply(state: state, info: info, for: descriptor)
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
    /// outcome, and reloads its state + info. Shared by the toggle and Cmd+O.
    func runQuickAction(_ descriptor: QuickActionDescriptor, intent: ActionIntent) {
        guard let adapter = ActionAdapterRegistry.adapter(for: descriptor.actionId) else {
            showBanner("\(descriptor.title) is not available", style: .info, duration: Banner.unavailable)
            return
        }

        // A toggle press means "the opposite of the state I am looking at", so
        // resolve it to an explicit target before it reaches the adapter:
        // apply(.toggle) flips the LIVE state, which does the opposite of what
        // the user asked whenever the panel is stale (the system changed while
        // the launcher was hidden). An unknown displayed state keeps the blind
        // toggle.
        var intent = intent
        if intent == .toggle {
            switch quickActionStates[descriptor.actionId] {
            case .on?: intent = .setOn(false)
            case .off?: intent = .setOn(true)
            default: break
            }
        }

        // Show the target immediately for instant feedback; the re-read below
        // confirms (and corrects it if the change did not take).
        if case .setOn(let on) = intent {
            quickActionStates[descriptor.actionId] = on ? .on : .off
        }

        Task {
            let outcome = await adapter.apply(intent)
            await MainActor.run { showOutcomeBanner(outcome, fallback: "\(descriptor.title) done") }
            await reloadQuickAction(descriptor)
        }
    }

    /// Connects/disconnects a list item (a paired device) when its row is
    /// clicked. Shows an immediate "Connecting to…" banner because the operation
    /// can take a moment; the outcome banner replaces it when it finishes.
    func activateQuickActionItem(_ descriptor: QuickActionDescriptor, item: QuickActionListItem) {
        guard let itemId = item.id,
            let adapter = ActionAdapterRegistry.adapter(for: descriptor.actionId)
        else { return }

        let disconnecting = item.on == true
        let progress = disconnecting ? "Disconnecting from \(item.label)…" : "Connecting to \(item.label)…"
        showBanner(progress, style: .info, duration: Banner.inProgress)

        Task {
            let outcome = await adapter.applyItem(itemId, intent: .toggle)
            await MainActor.run { showOutcomeBanner(outcome, fallback: "Done") }
            await reloadQuickAction(descriptor)
        }
    }

    /// Re-reads one action's live state + info after an apply, so the toggle and
    /// device list stay truthful without a full refresh.
    private func reloadQuickAction(_ descriptor: QuickActionDescriptor) async {
        let (state, info) = await readQuickAction(descriptor)
        await MainActor.run { apply(state: state, info: info, for: descriptor) }
    }

    /// Reads an action's live state and info from its adapter (or `.unavailable`
    /// when no adapter is registered on this OS). Shared by the initial load and
    /// the post-apply reload.
    private func readQuickAction(_ descriptor: QuickActionDescriptor) async -> (ActionState, [String: InfoValue]) {
        guard let adapter = ActionAdapterRegistry.adapter(for: descriptor.actionId) else {
            return (.unavailable("Not supported on this Mac"), [:])
        }
        let state = await adapter.state()
        let info = await adapter.info(keys: descriptor.info.map(\.valueKey))
        return (state, info)
    }

    /// Stores a read result into the panel state. Main-actor only.
    private func apply(state: ActionState, info: [String: InfoValue], for descriptor: QuickActionDescriptor) {
        quickActionStates[descriptor.actionId] = state
        quickActionInfo[descriptor.actionId] = info
    }

    private func showOutcomeBanner(_ outcome: ActionOutcome, fallback: String) {
        switch outcome {
        case .ok(let banner):
            showBanner(banner ?? fallback, style: .success, duration: Banner.success)
        case .failed(let message):
            showBanner(message, style: .error, duration: Banner.error)
        case .needsPermission(let message):
            showBanner(message, style: .info, duration: Banner.needsPermission)
        }
    }
}
