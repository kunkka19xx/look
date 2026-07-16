# Writing a Quick Action control

A **control** is a system feature Look can act on from its right-hand panel:
toggle Bluetooth, toggle Wi-Fi, switch appearance, and so on. This guide is for
contributors adding a new one. (Maintainers: the design rationale lives in the
internal `specs/quick-actions.md`.)

The goal of the framework is that a new control is a small, mostly-declarative
contribution: **one shared descriptor, one native adapter file, one registry
line.** You never touch the panel, keyboard, or rendering code.

## How the pieces split

Look targets macOS and linows (Linux/Windows). Reading and setting system state
has no cross-platform implementation, so we share the *declaration* and keep the
*execution* native:

| Piece | Location | Scope |
|-------|----------|-------|
| **Descriptor** — what it is: id, match, control kind, on/off labels, info fields | `core/qactions` catalog | shared, all OSes |
| **Adapter** — how it runs: read + set the OS state (`state()` / `apply()`) | macOS: `…/QuickActions/Controls/<Name>Control.swift`; linows: `…/src-tauri/src/qactions/controls/<name>.rs` (Linux) and `<name>_windows.rs` (Windows), each `cfg`-gated | native, per OS |
| **Registration** — wire the adapter to its action id | macOS: `…/QuickActions/ActionAdapterRegistry.swift`; linows: `qactions/mod.rs` `adapter()` | native, one line |

A control is searchable **and** actionable from its single descriptor; you do not
separately register it with the search engine. If an OS has no adapter for a
declared action, the panel still shows the info and marks the action unavailable
there, rather than the action vanishing inconsistently.

## File map

The three files you touch are grouped together; the rest is framework you leave
alone. (The repo organizes by layer, `Support/` vs `Views/`, so the framework
pieces sit with their peers rather than in one feature folder.)

You edit:

```
core/qactions/src/lib.rs                                  declare the descriptor + per-OS result binding
apps/macos/…/Support/QuickActions/
  Controls/<Name>Control.swift                            macOS adapter (copy Bluetooth)
  ActionAdapterRegistry.swift                             one line: "id": Control()
apps/linows/src-tauri/src/qactions/
  controls/<name>.rs                                      linows Linux adapter (copy bluetooth.rs)
  controls/<name>_windows.rs                              linows Windows adapter (copy bluetooth_windows.rs)
  controls/mod.rs                                         cfg-gate each per-OS module
  mod.rs                                                  one line per OS in adapter()
```

Framework, for reference only, do not edit:

```
apps/macos/…/Support/QuickActions/SystemControl.swift     the adapter contract
apps/macos/…/Support/QuickActions/QuickActionModels.swift decodes the descriptor
apps/macos/…/Support/UI/ToggleSwitch.swift                reusable switch component
apps/macos/…/Views/Launcher/QuickActionsSection.swift     renders the controls
apps/macos/…/Views/Launcher/LauncherView+QuickActions.swift  loads state, runs actions
bridge/ffi/src/qactions_api.rs                            exposes the catalog to Swift
apps/linows/src-tauri/src/qactions/mod.rs                 contract + Tauri commands (edit only adapter())
apps/linows/src/js/components/qactions.js                 renders controls, loads state, runs actions
```

## Steps

1. **Declare** the descriptor once in the shared `core/qactions` catalog: id,
   the control kind (toggle/button), on/off labels, and any info fields. Then
   bind the result id(s) that trigger it in the per-OS `binding_for` arms
   (result ids differ per OS: `setting:com.apple.bluetoothsettings` on macOS,
   `setting:bluetooth` on Linux). The key hint is derived from the control kind
   (a toggle shows `⌘O` / `Ctrl+O`), so you do not set it.
2. **Implement** the adapter for each OS you support. Copy the reference
   (`BluetoothControl.swift` on macOS, `controls/bluetooth.rs` on linows),
   rename the type, and fill in `state()` and `apply(_:)` (plus `info()` on
   linows if the descriptor declares info fields). Keep **all** OS-specific
   and private-API code inside this one file. An OS without an adapter shows
   the action as unavailable there; that is fine.
3. **Register** it: one line per OS - `ActionAdapterRegistry.adapters`
   (`"wifi": WiFiControl()`) on macOS, the `adapter()` match in
   `qactions/mod.rs` (`"wifi" => Some(&controls::wifi::WifiControl)`) on
   linows.

## Adapter contract

A control conforms to `SystemControl`:

- `state() async -> ActionState` — read current state for display. Return
  `.on` / `.off` for a toggle, `.value("…")` for a non-boolean control, or
  `.unavailable("reason")` when it does not apply on this machine.
- `apply(_ intent: ActionIntent) async -> ActionOutcome` — perform the change and
  report the outcome. It is best-effort: never throw, never block; surface
  problems as `.failed("…")` or `.needsPermission("…")`.

### Skeleton

```swift
import Foundation

/// Toggles and reports <feature>. Action id: `"<id>"`.
struct <Name>Control: SystemControl {
    func state() async -> ActionState {
        // Read current state; `.on` / `.off`, `.value("…")`,
        // or `.unavailable("reason")`.
    }

    func apply(_ intent: ActionIntent) async -> ActionOutcome {
        switch intent {
        case .toggle:        // flip it
        case .setOn(let on): // force on/off
        case .run:           return .failed("<feature> has no run action")
        }
        // Perform the change, then return one of:
        return .ok(banner: "…")          // success
        // return .failed("…")           // could not do it
        // return .needsPermission("…")  // OS permission required
    }
}
```

The linows contract is the Rust mirror of the same shape (see
`apps/linows/src-tauri/src/qactions/mod.rs`): `state()`, `apply(intent)`, and an
optional `info(keys)` that resolves the descriptor's info `value_key`s to
display values. Adapters may block (D-Bus, CLIs); the commands run them on the
blocking pool.

## Reference

Read [`BluetoothControl.swift`](../apps/macos/LauncherApp/look-app/Support/QuickActions/Controls/BluetoothControl.swift)
(macOS, quarantines a private API) or
[`bluetooth.rs`](../apps/linows/src-tauri/src/qactions/controls/bluetooth.rs)
(linows Linux, talks to BlueZ over D-Bus) first: each is a complete, commented
adapter and the template every other control follows.

### Windows Bluetooth

[`bluetooth_windows.rs`](../apps/linows/src-tauri/src/qactions/controls/bluetooth_windows.rs)
is the Windows peer. Power on/off goes through the WinRT
`Windows.Devices.Radios` API (the same surface as the OS Quick Settings toggle,
no elevation needed); WinRT calls block on `IAsyncOperation::get()`, which is
fine on the blocking pool. `ensure_mta()` keeps the process in an MTA so those
calls work on pooled threads. The paired-device list comes from WinRT
`DeviceInformation` (classic + LE).

Connect/disconnect has no WinRT equivalent of BlueZ's per-device
`Connect`/`Disconnect`, so `apply_item` drops to the Win32
`BluetoothSetServiceState` API (in the `winbt` module), which acts per installed
service. Only classic devices are actionable: their row `id` is the Bluetooth
address, which `apply_item` uses to find the `BLUETOOTH_DEVICE_INFO` and toggle
its services. LE devices have no `id` and stay display-only.
