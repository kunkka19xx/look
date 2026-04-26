import AppKit
import SwiftUI

extension LauncherView {
    @ViewBuilder
    var themedBackground: some View {
        if let image = themeStore.backgroundImage {
            backgroundImageView(image: image)
                .blur(radius: themeStore.settings.backgroundImageBlur)
                .opacity(themeStore.settings.backgroundImageOpacity)
        }

        VisualEffectBlur(material: themeStore.settings.blurMaterial.material)
            .opacity(
                min(
                    1,
                    max(
                        0,
                        themeStore.settings.blurOpacity
                            * themeStore.settings.blurMaterial.blurOpacityScale
                            * (appUIState.showsThemeSettings ? appUIState.settingsBlurMultiplier : 1.0)
                    )
                )
            )

        Color(
            .sRGB,
            red: themeStore.settings.tintRed,
            green: themeStore.settings.tintGreen,
            blue: themeStore.settings.tintBlue,
            opacity: min(
                1,
                max(
                    0,
                    themeStore.settings.tintOpacity * themeStore.settings.blurMaterial.tintOpacityScale
                )
            )
        )
    }

    @ViewBuilder
    func backgroundImageView(image: NSImage) -> some View {
        GeometryReader { proxy in
            let size = proxy.size

            Group {
                switch themeStore.settings.backgroundImageMode {
                case .fit:
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFit()
                        .frame(width: size.width, height: size.height)
                case .fill:
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFill()
                        .frame(width: size.width, height: size.height)
                        .clipped()
                case .stretch:
                    Image(nsImage: image)
                        .resizable()
                        .frame(width: size.width, height: size.height)
                case .tile:
                    Rectangle()
                        .fill(ImagePaint(image: Image(nsImage: image), scale: 0.3))
                        .frame(width: size.width, height: size.height)
                }
            }
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
    }
}
