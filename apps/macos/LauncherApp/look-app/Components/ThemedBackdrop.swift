import AppKit
import SwiftUI

/// Frost plus tint at the opacities set in Settings. Shared by the window
/// backdrop and every floating tile so the sliders reach all of them.
///
/// On macOS 26 the frost is Liquid Glass for every theme, with the slider
/// riding the glass tint; earlier systems fall back to a light material under
/// a scrim. Slider at 0 draws no frost at all.
struct ThemedBackdrop: View {
    @ObservedObject var themeStore: ThemeStore
    /// Thins the frost only, for the settings overlay.
    var blurOpacityMultiplier: Double = 1
    /// Fallback path only. The window backdrop frosts the desktop; a tile
    /// inside the window must not, or it samples past the panel it sits on.
    var blendingMode: NSVisualEffectView.BlendingMode = .withinWindow
    /// Corner the glass is cut to. Callers clip this view, but glass draws its
    /// own specular edge and needs the corner up front or it lands outside the
    /// clip. Ignored by the fallback path.
    var cornerRadius: CGFloat = 0

    /// The heaviest darkness a full slider adds over the base frost.
    private static let maxFrostScrim = 0.7
    /// Below this no frost is drawn: the fallback material cannot fade (it
    /// loses its blur below full alpha), so 0 means off on both paths.
    private static let frostCutoff = 0.01

    private var tintOpacity: Double {
        clamped(themeStore.settings.tintOpacity * themeStore.settings.blurMaterial.tintOpacityScale)
    }

    private var frostWeight: Double {
        clamped(
            themeStore.settings.blurOpacity
                * themeStore.settings.blurMaterial.blurOpacityScale
                * blurOpacityMultiplier
        )
    }

    private var showsFrost: Bool {
        frostWeight > Self.frostCutoff
    }

    /// Squared so the low end ramps gently while the range stays visible.
    private var frostScrimOpacity: Double {
        frostWeight * frostWeight * Self.maxFrostScrim
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
                GlassEffectBackdrop(
                    cornerRadius: cornerRadius,
                    tint: tintColor,
                    appearance: themeStore.themeAppearance())
                    .opacity(clamped(blurOpacityMultiplier))
            } else if #available(macOS 26.0, *) {
                if showsFrost {
                    GlassEffectBackdrop(
                        cornerRadius: cornerRadius,
                        tint: frostGlassTint,
                        appearance: themeStore.themeAppearance())
                } else {
                    themeTintWash
                }
            } else {
                if showsFrost {
                    VisualEffectBlur(
                        material: .underWindowBackground,
                        blendingMode: blendingMode,
                        appearance: themeStore.themeAppearance()
                    )

                    themeStore.scrimColor(opacity: frostScrimOpacity)
                }

                themeTintWash
            }
        }
    }

    private var themeTintWash: Color {
        Color(
            .sRGB,
            red: themeStore.settings.tintRed,
            green: themeStore.settings.tintGreen,
            blue: themeStore.settings.tintBlue,
            opacity: tintOpacity
        )
    }

    /// Theme tint composited under the slider's darkness, as the single colour
    /// the glass accepts: a wash layered over glass would cancel the
    /// refraction, so the two merge here instead.
    private var frostGlassTint: NSColor? {
        let scrim = frostScrimOpacity
        let tint = tintOpacity
        let outAlpha = scrim + tint * (1 - scrim)
        guard outAlpha > 0.001 else { return nil }

        let scrimChannel = themeStore.themeAppearance() == .dark ? 0.0 : 1.0
        func blend(_ themeChannel: Double) -> Double {
            (scrimChannel * scrim + themeChannel * tint * (1 - scrim)) / outAlpha
        }
        return NSColor(
            srgbRed: blend(themeStore.settings.tintRed),
            green: blend(themeStore.settings.tintGreen),
            blue: blend(themeStore.settings.tintBlue),
            alpha: outAlpha
        )
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
