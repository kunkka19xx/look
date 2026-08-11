import AppKit
import SwiftUI

/// Blur plus tint at the opacities set in Settings. Shared by the window backdrop
/// and every floating tile so both sliders reach all of them.
///
/// Liquid Glass takes a different route through this view: the glass carries the
/// tint itself and no wash is drawn over it, because a flat colour layer at any
/// usable opacity cancels the refraction. See `GlassEffectBackdrop`.
struct ThemedBackdrop: View {
    @ObservedObject var themeStore: ThemeStore
    /// Thins the blur only, for the settings overlay.
    var blurOpacityMultiplier: Double = 1
    /// Corner the Liquid Glass surface is cut to. Callers already clip this view,
    /// but glass draws its own specular edge and needs the corner up front or
    /// that edge lands outside the clip and is lost. Ignored by the blur path.
    var cornerRadius: CGFloat = 0

    private var tintOpacity: Double {
        clamped(themeStore.settings.tintOpacity * themeStore.settings.blurMaterial.tintOpacityScale)
    }

    var body: some View {
        backdrop
            // The backdrop never animates. It is nested inside the panel, so it
            // inherits any ambient transaction: arrow-key nav wraps its selection
            // assignment in a global `withAnimation` (LauncherView+Selection), and
            // re-compositing an NSVisualEffectView or NSGlassEffectView inside that
            // transaction flickers the entire window on every keypress.
            .transaction { $0.animation = nil }
    }

    @ViewBuilder
    private var backdrop: some View {
        ZStack {
            if themeStore.settings.blurMaterial == .liquidGlass, #available(macOS 26.0, *) {
                // Glass on its own, no blur substrate. A blur underneath was
                // tried and read as *less* liquid: the opaque frost plus a tint
                // wash sits in front of the refraction and flattens it. The
                // glass carries the tint itself instead.
                GlassEffectBackdrop(cornerRadius: cornerRadius, tint: tintColor)
                    .opacity(clamped(blurOpacityMultiplier))
            } else {
                VisualEffectBlur(
                    material: themeStore.settings.blurMaterial.material,
                    appearance: themeStore.themeAppearance()
                )
                .opacity(
                    clamped(
                        themeStore.settings.blurOpacity
                            * themeStore.settings.blurMaterial.blurOpacityScale
                            * blurOpacityMultiplier
                    )
                )

                Color(
                    .sRGB,
                    red: themeStore.settings.tintRed,
                    green: themeStore.settings.tintGreen,
                    blue: themeStore.settings.tintBlue,
                    opacity: tintOpacity
                )
            }
        }
    }

    /// The theme tint as an `NSColor` for the glass to absorb. nil at zero
    /// opacity so the glass stays untinted rather than multiplying by clear.
    private var tintColor: NSColor? {
        guard tintOpacity > 0 else { return nil }
        return NSColor(
            srgbRed: themeStore.settings.tintRed,
            green: themeStore.settings.tintGreen,
            blue: themeStore.settings.tintBlue,
            alpha: tintOpacity
        )
    }

    private func clamped(_ value: Double) -> Double {
        min(1, max(0, value))
    }
}
