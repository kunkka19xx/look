import AppKit
import SwiftUI

/// Unmodified single-key shortcuts for a panel that owns no text field, e.g.
/// `/speed`. The monitor lives only while the view is on screen and never takes
/// a keystroke aimed at a text editor, so it cannot fight the search field.
struct PlainKeyCommands: NSViewRepresentable {
    /// Lowercased characters mapped to what they run, e.g. `["r": rerun]`.
    var actions: [String: () -> Void]

    func makeNSView(context: Context) -> PlainKeyCommandsHostView {
        let view = PlainKeyCommandsHostView()
        view.actions = actions
        return view
    }

    func updateNSView(_ nsView: PlainKeyCommandsHostView, context: Context) {
        nsView.actions = actions
    }
}

final class PlainKeyCommandsHostView: NSView {
    var actions: [String: () -> Void] = [:]
    nonisolated(unsafe) private var monitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            removeMonitor()
        } else {
            installMonitor()
        }
    }

    deinit {
        // Inline so deinit stays nonisolated.
        if let monitor { NSEvent.removeMonitor(monitor) }
    }

    private func installMonitor() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self else { return event }
            if event.window?.firstResponder is NSText { return event }
            if let responder = event.window?.firstResponder as? NSView,
                responder.isKind(of: NSTextField.self) || responder.isKind(of: NSTextView.self)
            {
                return event
            }
            // A modifier means the key belongs to someone else (Cmd+R, and the
            // Cmd+Space launcher hotkey).
            guard event.modifierFlags.intersection(.deviceIndependentFlagsMask).isEmpty else {
                return event
            }
            guard let characters = event.charactersIgnoringModifiers?.lowercased(),
                let action = self.actions[characters]
            else {
                return event
            }
            action()
            return nil
        }
    }

    private func removeMonitor() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}
