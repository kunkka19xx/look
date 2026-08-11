import AppKit
import SwiftUI

/// AppKit's Liquid Glass surface, wrapped the same way as `VisualEffectBlur`.
///
/// SwiftUI's `glassEffect` refracts only what sits behind it *inside its own
/// view tree*. Look's window is transparent (`WindowConfigurator` sets
/// `isOpaque = false` with a clear background), so in the SwiftUI tree there is
/// nothing behind the backdrop and the effect renders as very nearly nothing.
/// `NSGlassEffectView` is the window-level primitive and composites the way the
/// system's own glass surfaces do, which is what a launcher floating over the
/// desktop needs.
@available(macOS 26.0, *)
struct GlassEffectBackdrop: NSViewRepresentable {
    var cornerRadius: CGFloat
    /// The theme tint, handed to the glass rather than layered over it. A flat
    /// colour wash on top at any usable opacity cancels the refraction and the
    /// surface collapses back to looking like a plain blur.
    var tint: NSColor?

    func makeNSView(context: Context) -> NSGlassEffectView {
        let view = NSGlassEffectView()
        view.style = .regular
        view.cornerRadius = cornerRadius
        view.tintColor = tint
        // The view exists to embed a content view in glass, and draws nothing
        // when it has none. Look wants the material on its own as a backdrop
        // layer, so it gets an empty transparent view to wrap.
        view.contentView = NSView()
        return view
    }

    func updateNSView(_ nsView: NSGlassEffectView, context: Context) {
        // Reassigning an unchanged value makes the glass recompute and flash,
        // the same reason `VisualEffectBlur` guards its material.
        if nsView.cornerRadius != cornerRadius {
            nsView.cornerRadius = cornerRadius
        }
        if nsView.tintColor != tint {
            nsView.tintColor = tint
        }
    }
}
