import SwiftUI

/// Blur plus tint at the opacities set in Settings. Shared by the window backdrop
/// and every floating tile so both sliders reach all of them.
struct ThemedBackdrop: View {
    @ObservedObject var themeStore: ThemeStore
    /// Thins the blur only, for the settings overlay.
    var blurOpacityMultiplier: Double = 1

    var body: some View {
        ZStack {
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
                opacity: clamped(
                    themeStore.settings.tintOpacity * themeStore.settings.blurMaterial.tintOpacityScale
                )
            )
        }
    }

    private func clamped(_ value: Double) -> Double {
        min(1, max(0, value))
    }
}
