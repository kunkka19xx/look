import AppKit
import SwiftUI

/// Hosts the launcher and skips cursor updates while the mouse is still.
///
/// A scrolling list moves rows under the pointer every frame, and AppKit sets
/// the cursor again for each. With a customized accessibility pointer, every
/// set rebuilds the cursor image on the main thread and stutters the scroll.
///
/// Not generic: the Release optimizer crashes on a generic subclass's deinit.
final class LauncherHostingView: NSHostingView<AnyView> {
    private var lastCursorUpdateLocation: NSPoint?

    override func cursorUpdate(with event: NSEvent) {
        let location = NSEvent.mouseLocation
        guard location != lastCursorUpdateLocation else { return }
        lastCursorUpdateLocation = location
        super.cursorUpdate(with: event)
    }
}
