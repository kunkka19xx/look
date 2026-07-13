import Foundation

/// Quick Actions framework - the single registration point for native control
/// adapters (see `specs/quick-actions.md`).
///
/// TO ADD A CONTROL: implement a `SystemControl` in `Controls/`, then add ONE
/// line to `adapters` below. That is the only edit outside your own file and the
/// shared `core/qactions` descriptor. If an action id has no adapter on this OS,
/// the panel still shows the info and marks the action unavailable.
enum ActionAdapterRegistry {
    /// Maps a descriptor's `action_id` (declared in the shared `core/qactions`
    /// catalog) to the native adapter that runs it on macOS.
    static let adapters: [String: any SystemControl] = [
        "bluetooth": BluetoothControl(),
    ]

    static func adapter(for actionID: String) -> (any SystemControl)? {
        adapters[actionID]
    }
}
