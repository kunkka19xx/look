import AppKit
import SwiftUI

/// Blur plus tint at the opacities set in Settings. Shared by the window backdrop
/// and every floating tile so both sliders reach all of them.
///
/// On Liquid Glass the glass carries the tint itself and no wash is drawn over
/// it. See `GlassEffectBackdrop`.
struct ThemedBackdrop: View {
    @ObservedObject var themeStore: ThemeStore
    /// Thins the blur only, for the settings overlay.
    var blurOpacityMultiplier: Double = 1
    /// Corner the glass is cut to. Callers clip this view, but glass draws its
    /// own specular edge and needs the corner up front or it lands outside the
    /// clip. Ignored by the blur path.
    var cornerRadius: CGFloat = 0

    private var tintOpacity: Double {
        clamped(themeStore.settings.tintOpacity * themeStore.settings.blurMaterial.tintOpacityScale)
    }

    var body: some View {
        backdrop
            // Never animates: nav wraps its selection assignment in a global
            // `withAnimation`, and re-compositing the effect view inside that
            // transaction flickers the whole window on every keypress.
            .transaction { $0.animation = nil }
    }

    @ViewBuilder
    private var backdrop: some View {
        ZStack {
            if themeStore.settings.blurMaterial == .liquidGlass, #available(macOS 26.0, *) {
                // No blur substrate: frost in front of the refraction flattens it.
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

    /// nil at zero opacity, so the glass stays untinted rather than
    /// multiplying by clear.
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
