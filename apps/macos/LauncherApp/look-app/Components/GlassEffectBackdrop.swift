import AppKit
import SwiftUI

/// AppKit's Liquid Glass surface, wrapped the same way as `VisualEffectBlur`.
///
/// Not SwiftUI's `glassEffect`: that refracts only what sits behind it inside
/// its own view tree, and Look's window is transparent, so it renders as nearly
/// nothing. `NSGlassEffectView` composites at the window level.
@available(macOS 26.0, *)
struct GlassEffectBackdrop: NSViewRepresentable {
    var cornerRadius: CGFloat
    /// Handed to the glass rather than layered over it: a colour wash on top at
    /// any usable opacity cancels the refraction.
    var tint: NSColor?

    func makeNSView(context: Context) -> NSGlassEffectView {
        let view = NSGlassEffectView()
        view.style = .regular
        view.cornerRadius = cornerRadius
        view.tintColor = tint
        // Draws nothing without a content view to embed.
        view.contentView = NSView()
        return view
    }

    func updateNSView(_ nsView: NSGlassEffectView, context: Context) {
        // Reassigning an unchanged value makes the glass recompute and flash.
        if nsView.cornerRadius != cornerRadius {
            nsView.cornerRadius = cornerRadius
        }
        if nsView.tintColor != tint {
            nsView.tintColor = tint
        }
    }
}
