import Foundation

/// Swift mirror of the shared `look_qactions` descriptor (see
/// `docs/writing-controls.md`). Decoded from the FFI JSON with
/// `.convertFromSnakeCase`, so `action_id` -> `actionId`, `on_label` ->
/// `onLabel`, etc. This is the declarative half; execution is a `SystemControl`
/// adapter resolved by `actionId` in `ActionAdapterRegistry`.

/// How the action's control renders in the panel.
enum QuickActionControlKind: String, Decodable {
    case toggle
    case button
}

/// A read-only field shown above the actions. `valueKey` is resolved to a live
/// value by the native adapter (the descriptor only declares the label + key).
struct QuickActionInfoField: Decodable, Equatable {
    let label: String
    let valueKey: String
}

/// A declared Quick Action for a result.
struct QuickActionDescriptor: Decodable, Equatable, Identifiable {
    let actionId: String
    let title: String
    let control: QuickActionControlKind
    let onLabel: String?
    let offLabel: String?
    let info: [QuickActionInfoField]
    /// A question to ask before running, already expanded against the row.
    ///
    /// Carried ON the descriptor rather than looked up when the key is pressed:
    /// a lookup can miss (a cleared cache, a load still in flight) and a missing
    /// question reads as "no confirmation needed", which would run a destructive
    /// action silently. The descriptor is what gets activated, so the guard
    /// travels with it.
    var confirm: String? = nil
    /// Chord that already runs this, shown right-aligned in the menu.
    var shortcut: String? = nil

    var id: String { actionId }
}
