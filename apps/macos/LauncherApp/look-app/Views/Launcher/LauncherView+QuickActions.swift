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

    /// The actions with something applying right now: the action itself (a toggle
    /// mid-flip) or one of its list items (a device connecting). One action at a time.
    ///
    /// Scoped to the action, not the whole panel. Powering the controller down while one
    /// of its devices is still connecting is a real race, so a busy action must not take
    /// another press. But a Bluetooth connect can take seconds, and freezing every other
    /// control for its duration would be wrong the moment a second adapter exists. linows
    /// locks globally; with Bluetooth as the only action today the two are
    /// indistinguishable, and this one stays right when that stops being true.
    ///
    /// Derived once here and handed to the view, so the guard and what the panel draws
    /// can never disagree about which controls are live.
    var busyQuickActionIds: Set<String> {
        Set(
            quickActionDescriptors
                .filter { descriptor in
                    pendingQuickActions.contains(descriptor.actionId)
                        || quickActionItemIds(descriptor).contains(where: pendingQuickActions.contains)
                }
                .map(\.actionId)
        )
    }

    /// Every list-item id this action currently shows (e.g. its paired devices).
    private func quickActionItemIds(_ descriptor: QuickActionDescriptor) -> [String] {
        (quickActionInfo[descriptor.actionId] ?? [:]).values.flatMap { value -> [String] in
            guard case .list(let items) = value else { return [] }
            return items.compactMap(\.id)
        }
    }

    /// Loads the selected result's Quick Actions and reads each one's live state
    /// and info off the main thread. Cancels any in-flight read so a stale result
    /// never populates the panel. Called on selection/query change.
    func refreshQuickActions() {
        quickActionTask?.cancel()

        guard let result = selectedResultForActions else {
            if !quickActionDescriptors.isEmpty { quickActionDescriptors = [] }
            if !quickActionStates.isEmpty { quickActionStates = [:] }
            if !quickActionInfo.isEmpty { quickActionInfo = [:] }
            quickActionsLoadedResultID = nil
            return
        }

        let descriptors = bridge.quickActions(forResultID: result.id, kind: result.kind.rawValue)
        quickActionDescriptors = descriptors

        // Only a *different* result invalidates what the panel is showing. Re-reading
        // the same one (a window show, a re-render) keeps its resolved state until the
        // fresh values land, so the toggle does not blink back to its dimmed loading
        // look every time, and the unresolved-state window that `togglePrimaryQuickAction`
        // has to refuse to act in stays as narrow as possible.
        //
        // Carrying state across a selection change would be the worse bug: the new
        // result's panel would show the previous result's toggle.
        if quickActionsLoadedResultID != result.id {
            quickActionStates = [:]
            quickActionInfo = [:]
            quickActionsLoadedResultID = result.id
        }

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

    /// Flips the selected result's primary toggle (Cmd+O). A toggle whose state has not
    /// resolved yet is refused by `runQuickAction`, the same way `ToggleSwitch` renders
    /// it dimmed and `.disabled` to a click.
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
        // Ignore a press while anything under this action is still applying, including
        // one of its devices connecting: flipping the controller off mid-connect races
        // the apply and the re-read that follows it.
        let key = descriptor.actionId
        guard !busyQuickActionIds.contains(key) else { return }
        let resultID = selectedResultID

        // A toggle press means "the opposite of the state I am looking at", so resolve
        // it to an explicit target before it reaches the adapter: apply(.toggle) flips
        // the LIVE state, which does the opposite of what the user asked whenever the
        // panel is stale (the system changed while the launcher was hidden).
        //
        // With no displayed state there is nothing to take the opposite of, so refuse
        // rather than fall back to that blind flip. Enforced here, at the one point every
        // caller passes through, so no future entry point can reintroduce it: the click
        // is already blocked upstream by `ToggleSwitch` rendering an unresolved state as
        // dimmed and `.disabled`, but Cmd+O reaches this function directly.
        var intent = intent
        if intent == .toggle {
            switch quickActionStates[descriptor.actionId] {
            case .on?: intent = .setOn(false)
            case .off?: intent = .setOn(true)
            default: return
            }
        }

        // Show the target immediately for instant feedback; the re-read below
        // confirms (and corrects it if the change did not take).
        if case .setOn(let on) = intent {
            quickActionStates[descriptor.actionId] = on ? .on : .off
        }

        pendingQuickActions.insert(key)
        Task {
            let outcome = await adapter.apply(intent)
            await MainActor.run {
                pendingQuickActions.remove(key)
                showOutcomeBanner(outcome, fallback: "\(descriptor.title) done")
            }
            await reloadQuickAction(descriptor, resultID: resultID)
        }
    }

    /// Connects/disconnects a list item (a paired device) when its row is
    /// clicked. Shows an immediate "Connecting to…" banner because the operation
    /// can take a moment; the outcome banner replaces it when it finishes.
    func activateQuickActionItem(_ descriptor: QuickActionDescriptor, item: QuickActionListItem) {
        guard let itemId = item.id,
            let adapter = ActionAdapterRegistry.adapter(for: descriptor.actionId),
            // Ignore a re-click on this row, and a click on a sibling row or the toggle,
            // while anything under this action is applying.
            !busyQuickActionIds.contains(descriptor.actionId)
        else { return }
        let resultID = selectedResultID

        let disconnecting = item.on == true
        let progress = disconnecting ? "Disconnecting from \(item.label)…" : "Connecting to \(item.label)…"
        showBanner(progress, style: .info, duration: Banner.inProgress)

        pendingQuickActions.insert(itemId)
        Task {
            let outcome = await adapter.applyItem(itemId, intent: .toggle)
            await MainActor.run {
                pendingQuickActions.remove(itemId)
                showOutcomeBanner(outcome, fallback: "Done")
            }
            await reloadQuickAction(descriptor, resultID: resultID)
        }
    }

    /// Re-reads one action's live state + info after an apply, so the toggle and
    /// device list stay truthful without a full refresh. Drops the write if the
    /// selection changed while the (slow) apply was in flight.
    private func reloadQuickAction(_ descriptor: QuickActionDescriptor, resultID: String?) async {
        let (state, info) = await readQuickAction(descriptor)
        await MainActor.run {
            guard selectedResultID == resultID else { return }
            apply(state: state, info: info, for: descriptor)
        }
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
