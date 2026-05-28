import AppKit
import SwiftUI

/// Invisible NSView whose only job is to return
/// `mouseDownCanMoveWindow = true` so AppKit drags the launcher window
/// when a mouseDown lands on the view directly (i.e. no SwiftUI gesture
/// upstream claimed the event). Used as the background of the
/// running-apps strip area; without it the strip's spacing/padding is
/// unreachable as a window-drag handle because SwiftUI's tap/hover
/// gestures on each icon consume mouseDown.
///
/// NOTE: This is the *limited* drag fix — it only works where SwiftUI
/// doesn't claim the event (strip padding + bordered-panel padding).
/// The deeper "drag from anywhere, including across vertically-stacked
/// monitors" problem is parked — see docs/tasks.md "macOS launcher
/// drag" entry for the open work.
struct WindowDragArea: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { MouseDownDraggableView() }
    func updateNSView(_ nsView: NSView, context: Context) {}
}

private final class MouseDownDraggableView: NSView {
    override var mouseDownCanMoveWindow: Bool { true }
}
