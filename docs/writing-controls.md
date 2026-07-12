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
| **Adapter** — how it runs: read + set the OS state (`state()` / `apply()`) | `apps/<platform>/…/QuickActions/Controls/<Name>Control.swift` | native, per OS |
| **Registration** — wire the adapter to its action id | `…/QuickActions/ActionAdapterRegistry.swift` | native, one line |

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
core/qactions/src/lib.rs                                  declare the descriptor
apps/macos/…/Support/QuickActions/
  Controls/<Name>Control.swift                            your adapter (copy Bluetooth)
  ActionAdapterRegistry.swift                             one line: "id": Control()
```

Framework, for reference only, do not edit:

```
apps/macos/…/Support/QuickActions/SystemControl.swift     the adapter contract
apps/macos/…/Support/QuickActions/QuickActionModels.swift decodes the descriptor
apps/macos/…/Support/UI/ToggleSwitch.swift                reusable switch component
apps/macos/…/Views/Launcher/QuickActionsSection.swift     renders the controls
apps/macos/…/Views/Launcher/LauncherView+QuickActions.swift  loads state, runs actions
bridge/ffi/src/qactions_api.rs                            exposes the catalog to Swift
```

## Steps

1. **Declare** the descriptor once in the shared `core/qactions` catalog: id,
   what result it matches (`setting:<x>`, an app kind, a bundle id), the control
   kind (toggle/button), on/off labels, and any info fields. The key hint is
   derived from the control kind (a toggle shows `⌘O`), so you do not set it.
2. **Implement** the adapter. Copy the reference (`BluetoothControl.swift`),
   rename the type, and fill in `state()` and `apply(_:)`. Keep **all**
   OS-specific and private-API code inside this one file.
3. **Register** it: add one line to `ActionAdapterRegistry.adapters`, e.g.
   `"wifi": WiFiControl()`.

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

## Reference

Read [`BluetoothControl.swift`](../apps/macos/LauncherApp/look-app/Support/QuickActions/Controls/BluetoothControl.swift)
first: it is a complete, commented adapter (including how it quarantines a private
macOS API) and the template every other control follows.
