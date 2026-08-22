import AppKit
import SwiftUI

struct VisualEffectBlur: NSViewRepresentable {
    var material: NSVisualEffectView.Material
    /// What the material samples. `.behindWindow` frosts the desktop, which is
    /// the look the material is for; `.withinWindow` samples the window's own
    /// backing, so with nothing behind it renders as a flat wash whose only
    /// adjustable property is alpha.
    var blendingMode: NSVisualEffectView.BlendingMode = .withinWindow
    /// Pinned per theme: materials otherwise follow the system light/dark setting.
    var appearance: ThemeAppearance = .dark

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = material
        view.blendingMode = blendingMode
        view.state = .active
        view.appearance = NSAppearance(named: appearance.nsAppearanceName)
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {
        // Reassigning the same material forces the blur to recompute
        // and produces a brief brighter→darker flash. Skip if it
        // hasn't actually changed.
        if nsView.material != material {
            nsView.material = material
        }
        if nsView.blendingMode != blendingMode {
            nsView.blendingMode = blendingMode
        }
        if nsView.appearance?.name != appearance.nsAppearanceName {
            nsView.appearance = NSAppearance(named: appearance.nsAppearanceName)
        }
    }
}
