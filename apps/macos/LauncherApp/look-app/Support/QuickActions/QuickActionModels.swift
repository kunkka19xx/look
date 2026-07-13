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

    var id: String { actionId }
}
