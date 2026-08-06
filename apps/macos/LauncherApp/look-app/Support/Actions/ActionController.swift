import Combine
import Foundation

/// Runtime state for the Act pillar: holds the pending action awaiting confirm,
/// runs it, and keeps the last receipt for undo. Both producers (the `>` parser
/// and, later, the model planner) call `propose`.
@MainActor
final class ActionController: ObservableObject {
    static let shared = ActionController()

    @Published private(set) var pending: PlannedAction?
    @Published private(set) var lastReceipt: ActionReceipt?
    @Published private(set) var feedback: String = ""
    /// True while the model is turning a `>` query into an action, so the UI can
    /// show a "thinking" indicator during the generation.
    @Published private(set) var isPlanning: Bool = false

    private let registry: ActionRegistry
    private let planner: ActionPlanner
    private var planTask: Task<Void, Never>?
    private var idleTask: Task<Void, Never>?
    private var lastWarm = Date.distantPast

    /// How long the user must stop typing before the model runs. Short, because
    /// an in-flight call is cancelled cleanly on the next keystroke (Task cancel
    /// closes the connection; Ollama aborts generation on disconnect). The
    /// working indicator shows instantly regardless.
    private static let idleDelay: UInt64 = 300_000_000

    private init() {
        let registry = ActionRegistry()
        registry.register(CalendarAddEventTool(store: EventKitService.shared))
        registry.register(ReminderAddTool(store: EventKitService.shared))
        self.registry = registry
        self.planner = ActionPlanner(registry: registry)
    }

    var isPresenting: Bool { pending != nil }

    /// Live preview while typing a `>` query. Two stages: an instant deterministic
    /// preview (the verb picks the action, the parser fills the spec) that tracks
    /// each keystroke with no model, plus a background model refinement that fires
    /// only once the user stops typing (idle) and upgrades the preview in place.
    func previewExplicitAIQuery(_ rawQuery: String) {
        idleTask?.cancel()
        planTask?.cancel()
        let query = stripPrefix(rawQuery)

        guard !query.isEmpty else {
            if isPresenting || isPlanning { cancel() }
            return
        }

        // Explicit `@` form: instant, exact, no model.
        if let call = ExplicitActionParser.parse(">" + query),
           case .planned(let action) = registry.plan(call, now: Date()) {
            isPlanning = false
            pending = action
            feedback = ""
            return
        }

        // Natural language: never show a rough/wrong title. Instead show the
        // working indicator IMMEDIATELY (no dead air) and normalize in the
        // background; the clean preview replaces the indicator when ready.
        guard planner.isAvailable else {
            pending = nil
            isPlanning = false
            feedback = "Connect a model, or use: >add <title> @ <time>"
            return
        }
        pending = nil
        isPlanning = true
        // Warm the model + prompt cache now, while the user is still typing, so
        // the first real plan skips model load and prompt processing. Throttled.
        if Date().timeIntervalSince(lastWarm) > 90 {
            lastWarm = Date()
            Task { [planner] in await planner.warmUp() }
        }
        idleTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.idleDelay)
            guard let self, !Task.isCancelled else { return }
            let call = await self.planner.plan(query: query)
            guard !Task.isCancelled else { return }  // superseded: newer task owns state
            self.isPlanning = false
            if let call {
                self.propose(call)
            } else {
                self.feedback = "Not a calendar or reminder action."
            }
        }
    }

    /// Enter with no preview yet: force an immediate plan (deterministic first,
    /// then model). When a preview is already showing, handleSubmit confirms it
    /// instead of calling this.
    func submitExplicitAIQuery(_ rawQuery: String) {
        idleTask?.cancel()
        let query = stripPrefix(rawQuery)
        guard !query.isEmpty else { return }

        if let call = ExplicitActionParser.parse(">" + query),
           case .planned(let action) = registry.plan(call, now: Date()) {
            planTask?.cancel()
            isPlanning = false
            pending = action
            feedback = ""
            return
        }

        guard planner.isAvailable else {
            feedback = "Couldn't parse. Try: >add <title> @ <time>"
            return
        }
        planTask?.cancel()
        planTask = Task { [weak self] in
            guard let self else { return }
            self.isPlanning = true
            let call = await self.planner.plan(query: query)
            guard !Task.isCancelled else {
                self.isPlanning = false
                return
            }
            self.isPlanning = false
            if let call {
                self.propose(call)
            } else {
                self.feedback = "Couldn't turn that into an action."
            }
        }
    }

    private func stripPrefix(_ raw: String) -> String {
        let text = raw.hasPrefix(">") ? String(raw.dropFirst()) : raw
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func propose(_ call: ToolCall) {
        switch registry.plan(call, now: Date()) {
        case .planned(let action):
            pending = action
            feedback = ""
        case .invalid(let message):
            pending = nil
            feedback = message
        case .needsChoice:
            pending = nil
            feedback = "Multiple matches. Be more specific."
        }
    }

    /// Confirm the pending action. Requests calendar/reminder access on first use
    /// (the only place a permission prompt appears, besides the Settings button).
    func confirm() {
        guard let action = pending else { return }
        pending = nil
        Task {
            await ensureAccess(for: action.toolID)
            do {
                let receipt = try action.perform()
                lastReceipt = receipt
                feedback = receipt.summary
            } catch {
                feedback = "Failed: \(error.localizedDescription)"
            }
        }
    }

    func cancel() {
        planTask?.cancel()
        idleTask?.cancel()
        pending = nil
        feedback = ""
        isPlanning = false
    }

    func undoLast() {
        guard let receipt = lastReceipt else { return }
        do {
            try receipt.undo()
            feedback = "Undone"
        } catch {
            feedback = "Undo failed"
        }
        lastReceipt = nil
    }

    private func ensureAccess(for toolID: String) async {
        let access = toolID.hasPrefix("reminder")
            ? EventKitService.shared.reminderAccess
            : EventKitService.shared.calendarAccess
        if access == .notDetermined {
            await EventKitService.shared.requestAccess()
        }
    }
}
