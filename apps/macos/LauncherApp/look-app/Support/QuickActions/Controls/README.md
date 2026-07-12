# Controls

Per-control adapters for the Quick Actions panel. Each file is one
`SystemControl` (read + set some system state); the framework around them
(`../SystemControl.swift`, `../ActionAdapterRegistry.swift`, the panel, the
keyboard) is written once and you should not need to touch it.

**Adding a control:** see [docs/writing-controls.md](../../../../../../../docs/writing-controls.md).

`BluetoothControl.swift` is the reference implementation — read it first.
