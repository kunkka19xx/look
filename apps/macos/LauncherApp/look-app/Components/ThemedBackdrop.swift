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
        ZStack {
            if themeStore.settings.blurMaterial == .liquidGlass, #available(macOS 26.0, *) {
                // No blur-opacity term: dimming glass makes it ghostly rather
                // than lighter. Only the settings-overlay multiplier applies.
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
