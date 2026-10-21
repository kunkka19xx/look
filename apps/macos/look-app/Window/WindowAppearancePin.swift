import AppKit
import SwiftUI

/// Pins the window's NSAppearance to the theme's. Pickers, text fields and
/// sliders colour themselves from the window, not from the theme tokens: a light
/// theme otherwise drew a black text field and an unreadable popup on paper.
struct WindowAppearancePin: NSViewRepresentable {
    var appearance: ThemeAppearance

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            apply(to: view)
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            apply(to: nsView)
        }
    }

    private func apply(to view: NSView) {
        guard let window = view.window else {
            return
        }
        let name = appearance.nsAppearanceName
        guard window.appearance?.name != name else {
            return
        }
        window.appearance = NSAppearance(named: name)
    }
}
