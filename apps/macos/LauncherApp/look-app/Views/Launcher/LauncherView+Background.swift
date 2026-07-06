import AppKit
import SwiftUI

extension LauncherView {
    @ViewBuilder
    var themedBackground: some View {
        // Settings panel is rendered as an overlay over command mode
        // (via appUIState.showsThemeSettings) without flipping
        // isCommandMode. It needs the full themed bg, so prefer the
        // settings condition over the command-mode one.
        if isCommandMode && !appUIState.showsThemeSettings {
            // Command mode renders dynamic content (pomo timer, sys
            // refresh) with brightly-colored controls (Pause = warning
            // yellow, Reset = danger red, etc.). NSVisualEffectView with
            // .withinWindow blending was producing a soft halo of those
            // button colors behind/around the buttons - visible as a
            // yellow/red "eclipse blur" - because macOS samples the
            // window backing during recomposition and feeds it through
            // the blur pipeline.
            //
            // Replace it with a solid opaque tinted color in command
            // mode. Visually close to the frosted look (dark base with
            // the user's chosen tint applied) but with no
            // NSVisualEffectView to sample anything → no halo.
            commandModeBackdrop
        } else {
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
    }

    private var commandModeBackdrop: some View {
        themeStore.commandModeBackgroundColor()
    }

    @ViewBuilder
    func backgroundImageView(image: NSImage) -> some View {
        // Keep this in a GeometryReader so the image's frame preference
        // is contained - without it, `.frame(maxWidth: .infinity, ...)`
        // on the Image propagates upward through the ZStack and pushes
        // the launcher's inner content layout past the visible window
        // edge (preview pane gets clipped on the right).
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
