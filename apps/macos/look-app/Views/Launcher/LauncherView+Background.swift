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
            ThemedBackdrop(
                themeStore: themeStore,
                blurOpacityMultiplier: appUIState.showsThemeSettings
                    ? themeStore.settings.settingsBlurMultiplier : 1.0,
                blendingMode: .behindWindow,
                cornerRadius: themeStore.panelRadius
            )

            // Above the backdrop, not below it: the blur samples what is behind
            // the WINDOW, so anything drawn under it is simply covered. The
            // image keeps its own blur and opacity, and a partly transparent
            // one still shows the frosted desktop through.
            if let image = themeStore.backgroundImage {
                backgroundImageView(image: image)
                    .blur(radius: themeStore.settings.backgroundImageBlur)
                    .opacity(themeStore.settings.backgroundImageOpacity)
            }
        }
    }

    private var commandModeBackdrop: some View {
        themeStore.commandModeBackgroundColor()
    }

    func backgroundImageView(image: NSImage) -> some View {
        BackgroundImageView(themeStore: themeStore, image: image)
    }
}

/// The background picture at the size it is given, in the mode the user chose.
///
/// Lives outside `LauncherView` because the floating cards and the launchpad
/// tiles draw their own slice of the same picture, and a tile is not inside
/// that view.
struct BackgroundImageView: View {
    let themeStore: ThemeStore
    let image: NSImage

    private static let tileScale: CGFloat = 0.3

    var body: some View {
        // Kept in a GeometryReader so the image's frame preference is
        // contained - without it, `.frame(maxWidth: .infinity, ...)` on the
        // Image propagates upward through the ZStack and pushes the launcher's
        // inner content past the visible window edge (the preview pane gets
        // clipped on the right).
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
                        .fill(ImagePaint(image: Image(nsImage: image), scale: Self.tileScale))
                        .frame(width: size.width, height: size.height)
                }
            }
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
    }
}

/// Where the panel is, for a surface that wants its own slice of the picture.
/// Nil while the cards sit in one box: the window draws the picture once there,
/// and a slice on top of it would be the same picture twice.
struct PanelGeometry: Equatable {
    let size: CGSize
    let coordinateSpace: String
}

extension EnvironmentValues {
    @Entry var panelGeometry: PanelGeometry?
}

/// One surface's slice of the picture: the whole thing laid out at panel size
/// and pushed by where this surface sits, so the cards read as one picture cut
/// apart by the gaps rather than a copy each.
struct BackgroundImageSlice: View {
    let themeStore: ThemeStore
    let image: NSImage
    let panel: PanelGeometry

    var body: some View {
        GeometryReader { geo in
            let frame = geo.frame(in: .named(panel.coordinateSpace))
            BackgroundImageView(themeStore: themeStore, image: image)
                .frame(width: panel.size.width, height: panel.size.height)
                .offset(x: -frame.minX, y: -frame.minY)
                .blur(radius: themeStore.settings.backgroundImageBlur)
        }
    }
}
