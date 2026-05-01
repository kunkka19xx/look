import AppKit
import SwiftUI

struct VisualEffectBlur: NSViewRepresentable {
    var material: NSVisualEffectView.Material

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = material
        view.blendingMode = .withinWindow
        view.state = .active
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {
        // Reassigning the same material forces the blur to recompute
        // and produces a brief brighter→darker flash. Skip if it
        // hasn't actually changed.
        if nsView.material != material {
            nsView.material = material
        }
    }
}
