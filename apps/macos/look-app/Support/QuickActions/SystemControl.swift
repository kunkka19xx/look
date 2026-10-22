import Foundation

/// Quick Actions framework — Layer 1 contract (see `specs/quick-actions.md`).
///
/// A `SystemControl` is the ONE piece a contributor writes to add a new
/// actionable control (a settings toggle, an app action). It reads the current
/// state for display and applies an intent when the user runs the action. All
/// OS-specific and private-API code is quarantined inside the conforming type;
/// nothing else in the framework (panel, keyboard, registry) needs to change.
///
/// `BluetoothControl` is the reference implementation - copy it to add a control.

/// Current state of a control's value, read for display in the panel.
enum ActionState: Equatable {
    case on
    case off
    /// A non-boolean value shown as-is (e.g. a level or a mode name).
    case value(String)
    /// The control cannot act here: no hardware, unsupported OS, or blocked.
    /// The associated string is a short human reason shown in the panel.
    case unavailable(String)
}

/// What the user asked a control to do.
enum ActionIntent: Equatable {
    /// Flip a boolean control (on <-> off).
    case toggle
    /// Force a boolean control to a specific value.
    case setOn(Bool)
    /// Trigger a non-toggle action (a plain button).
    case run
}

/// Result of applying an intent, surfaced to the user as a banner.
enum ActionOutcome: Equatable {
    /// Success. Optional banner text; nil shows a default confirmation.
    case ok(banner: String?)
    /// The action failed; the string is shown to the user.
    case failed(String)
    /// The action needs an OS permission; the string guides the user to grant it.
    case needsPermission(String)
}

/// One entry in an `InfoValue.list`. An `id` makes the row actionable via
/// `SystemControl.applyItem` (e.g. connect/disconnect a device); `on` drives an
/// on/off marker (e.g. whether a device is connected). Both optional so a list
/// can be plain, read-only rows.
struct QuickActionListItem: Equatable, Hashable {
    let id: String?
    let label: String
    let on: Bool?
}

/// A resolved info-field value. The shared descriptor declares `label` +
/// `valueKey`; the adapter resolves the key to what to display: a single line
/// (`text`), one row per item (`list`, e.g. paired devices), or `unavailable`.
enum InfoValue: Equatable {
    case text(String)
    case list([QuickActionListItem])
    case unavailable(String)
}

/// The adapter a contributor implements per control. Keep it small and pure:
/// read state, apply an intent, return an outcome. Async so controls backed by
/// AppleScript, a CLI, or the network can await without blocking the UI; simple
/// controls (like Bluetooth) satisfy it trivially.
protocol SystemControl: Sendable {
    /// Read the current state for display. Return `.unavailable(reason)` when the
    /// control does not apply on this machine.
    func state() async -> ActionState

    /// Resolve the descriptor's info `valueKey`s to display values. Controls
    /// without info fields keep the default (no info shown).
    func info(keys: [String]) async -> [String: InfoValue]

    /// Perform `intent` and report the outcome. Best-effort: never throw, never
    /// block; surface problems as `.failed` / `.needsPermission`.
    func apply(_ intent: ActionIntent) async -> ActionOutcome

    /// Act on one item of a list-valued info field (e.g. connect/disconnect a
    /// specific device). Defaults to unsupported: most controls have no per-item
    /// actions.
    func applyItem(_ itemId: String, intent: ActionIntent) async -> ActionOutcome
}

extension SystemControl {
    func info(keys: [String]) async -> [String: InfoValue] { [:] }

    func applyItem(_ itemId: String, intent: ActionIntent) async -> ActionOutcome {
        .failed("This action has no per-item actions")
    }
}
