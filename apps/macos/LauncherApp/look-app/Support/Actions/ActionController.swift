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

    private let registry: ActionRegistry

    private init() {
        let registry = ActionRegistry()
        registry.register(CalendarAddEventTool(store: EventKitService.shared))
        registry.register(ReminderAddTool(store: EventKitService.shared))
        self.registry = registry
    }

    var isPresenting: Bool { pending != nil }

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
        pending = nil
        feedback = ""
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
