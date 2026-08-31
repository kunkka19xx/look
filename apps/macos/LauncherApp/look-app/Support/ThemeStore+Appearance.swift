import SwiftUI

extension ThemeStore {
    /// Bases for the opaque command-mode surfaces. Dark adds the tint to a
    /// near-black base; on paper that would clip past white, so light mixes.
    private enum CommandSurface {
        static let darkBackgroundBase = 0.18
        static let darkBackgroundBlueBase = 0.20
        static let darkPanelBase = 0.13
        static let darkPanelBlueBase = 0.15
        static let lightBackgroundBase = 0.94
        static let lightPanelBase = 0.87
        static let tintWeight = 0.25
    }

    // MARK: - Appearance Tokens

    func fontColor(opacityMultiplier: Double = 1.0) -> Color {
        let alpha = min(1, max(0, settings.fontOpacity * opacityMultiplier))
        return Color(red: settings.fontRed, green: settings.fontGreen, blue: settings.fontBlue, opacity: alpha)
    }

    func secondaryTextColor() -> Color {
        // Try theme's color first if set, otherwise derive from main text
        if let token = activeAppearanceStyle()?.textSecondary {
            return color(from: token, opacity: settings.fontOpacity)
        }
        return dimmableColor(baseColor: fontColor(), factor: 0.82)
    }

    func mutedTextColor() -> Color {
        if let token = activeAppearanceStyle()?.textMuted {
            return color(from: token, opacity: settings.fontOpacity * 0.78)
        }
        return dimmableColor(baseColor: fontColor(), factor: 0.64)
    }

    /// Muted everywhere except on glass. Glass keeps whatever is on the desktop
    /// visible through it, so the dimmest text in the app loses against a white
    /// document. Steps up to secondary there.
    func placeholderTextColor() -> Color {
        settings.blurMaterial.rendersGlass ? secondaryTextColor() : mutedTextColor()
    }

    func panelFillColor() -> Color {
        if let style = activeAppearanceStyle(), let token = style.panelFill {
            return color(from: token, opacity: style.panelFillOpacity)
        }
        return Color(red: 0.10, green: 0.10, blue: 0.12, opacity: 0.30)
    }

    func controlFillColor() -> Color {
        if let style = activeAppearanceStyle(), let token = style.controlFill {
            return color(from: token, opacity: style.controlFillOpacity)
        }
        return Color(red: 0.18, green: 0.18, blue: 0.20, opacity: 0.30)
    }

    /// A plate drawn inside an already-materialized panel: chat bubbles, the
    /// thinking/stop bars, note pills, key caps. Scaled per material so Liquid
    /// Glass keeps refracting instead of being covered by stacked fills.
    func surfaceFill(_ opacity: Double = 1) -> Color {
        controlFillColor().opacity(surfaceOpacity(opacity))
    }

    /// The scaled opacity itself, for surfaces that need their own colour (the
    /// code block's darkening plate) rather than the control fill.
    func surfaceOpacity(_ opacity: Double) -> Double {
        min(1, max(0, opacity * settings.blurMaterial.surfaceOpacityScale))
    }

    func dividerColor() -> Color {
        if let style = activeAppearanceStyle(), let token = style.divider {
            return color(from: token, opacity: style.dividerOpacity)
        }
        return Color(red: 0.40, green: 0.40, blue: 0.44, opacity: 0.20)
    }

    func selectionFillColor() -> Color {
        if let style = activeAppearanceStyle(), let token = style.selectionFill {
            return color(from: token, opacity: style.selectionFillOpacity)
        }
        return Color(red: 0.50, green: 0.50, blue: 0.58, opacity: 0.25)
    }

    func accentColor() -> Color {
        if let token = activeAppearanceStyle()?.accent {
            return color(from: token, opacity: 1.0)
        }
        return fontColor(opacityMultiplier: 0.95)
    }

    func onAccentColor() -> Color {
        if let token = activeAppearanceStyle()?.onAccent {
            return color(from: token, opacity: 1.0)
        }
        if let accent = activeAppearanceStyle()?.accent {
            return contrastingTextColor(for: accent)
        }
        return .white
    }

    func successColor() -> Color {
        if let token = activeAppearanceStyle()?.success {
            return color(from: token, opacity: 1.0)
        }
        return Color(red: 0.65, green: 0.90, blue: 0.62, opacity: 1.0)
    }

    func onSuccessColor() -> Color {
        if let token = activeAppearanceStyle()?.success {
            return contrastingTextColor(for: token)
        }
        return .white
    }

    func warningColor() -> Color {
        if let token = activeAppearanceStyle()?.warning {
            return color(from: token, opacity: 1.0)
        }
        return Color(red: 0.96, green: 0.86, blue: 0.66, opacity: 1.0)
    }

    func onWarningColor() -> Color {
        if let token = activeAppearanceStyle()?.warning {
            return contrastingTextColor(for: token)
        }
        return .black
    }

    func dangerColor() -> Color {
        if let token = activeAppearanceStyle()?.danger {
            return color(from: token, opacity: 1.0)
        }
        return Color(red: 0.94, green: 0.50, blue: 0.55, opacity: 1.0)
    }

    func onDangerColor() -> Color {
        if let token = activeAppearanceStyle()?.danger {
            return contrastingTextColor(for: token)
        }
        return .white
    }

    // Command-mode panels render against an opaque backdrop (no
    // visualEffect blur, no bg image) so we need solid theme-derived
    // colors. Both the outer backdrop and the inner card colors share
    // the same tint contribution so they read as one continuous surface
    // - the card is just a few points darker, like a subtle recess.

    func commandModeBackgroundColor() -> Color {
        commandSurfaceColor(
            darkBase: ThemeRGB(
                red: CommandSurface.darkBackgroundBase,
                green: CommandSurface.darkBackgroundBase,
                blue: CommandSurface.darkBackgroundBlueBase
            ),
            lightBase: CommandSurface.lightBackgroundBase
        )
    }

    func commandModePanelColor() -> Color {
        commandSurfaceColor(
            darkBase: ThemeRGB(
                red: CommandSurface.darkPanelBase,
                green: CommandSurface.darkPanelBase,
                blue: CommandSurface.darkPanelBlueBase
            ),
            lightBase: CommandSurface.lightPanelBase
        )
    }

    /// Wash that seats a pane on the backdrop: darkens on dark, lightens on paper.
    func scrimColor(opacity: Double) -> Color {
        switch themeAppearance() {
        case .dark:
            return .black.opacity(opacity)
        case .light:
            return .white.opacity(opacity)
        }
    }

    /// The inverse of `scrimColor`, for chips and badges that must stand off
    /// the backdrop: white on dark, ink on paper.
    func liftColor(opacity: Double) -> Color {
        switch themeAppearance() {
        case .dark:
            return .white.opacity(opacity)
        case .light:
            return .black.opacity(opacity)
        }
    }

    func themeAppearance() -> ThemeAppearance {
        activeAppearanceStyle()?.appearance ?? .dark
    }

    /// Scales a surface's resting corner radius. One user setting for every
    /// surface, so they cannot disagree with each other.
    func surfaceCornerRadius(_ base: CGFloat) -> CGFloat {
        base * CGFloat(settings.surfaceRadius)
    }

    func borderColor() -> Color {
        Color(
            red: settings.borderRed,
            green: settings.borderGreen,
            blue: settings.borderBlue,
            opacity: settings.borderOpacity
        )
    }

    func borderLineWidth() -> CGFloat {
        CGFloat(max(0, settings.borderThickness))
    }

    // MARK: - Preset Resolution

    func applyBuiltinTheme(_ preset: BuiltinThemePreset) {
        guard let style = preset.style else {
            // Custom owns no palette: clearing the name is what makes the
            // semantic tokens fall back to being derived from the user's colors.
            settings.themeName = ""
            return
        }
        style.apply(to: &settings)
    }

    func detectBuiltinTheme(for settings: ThemeSettings) -> BuiltinThemePreset {
        if let named = BuiltinThemePreset.preset(forThemeName: settings.themeName) {
            return named
        }
        for preset in BuiltinThemePreset.allCases where preset != .custom {
            if let style = preset.style, style.matches(settings) {
                return preset
            }
        }
        return .custom
    }

    private func commandSurfaceColor(darkBase: ThemeRGB, lightBase: Double) -> Color {
        switch themeAppearance() {
        case .dark:
            return Color(
                .sRGB,
                red: darkBase.red + settings.tintRed * CommandSurface.tintWeight,
                green: darkBase.green + settings.tintGreen * CommandSurface.tintWeight,
                blue: darkBase.blue + settings.tintBlue * CommandSurface.tintWeight,
                opacity: 1.0
            )
        case .light:
            return Color(
                .sRGB,
                red: mixedWithTint(lightBase, settings.tintRed),
                green: mixedWithTint(lightBase, settings.tintGreen),
                blue: mixedWithTint(lightBase, settings.tintBlue),
                opacity: 1.0
            )
        }
    }

    private func mixedWithTint(_ base: Double, _ tint: Double) -> Double {
        base * (1 - CommandSurface.tintWeight) + tint * CommandSurface.tintWeight
    }

    private func activeAppearanceStyle() -> BuiltinThemeStyle? {
        detectBuiltinTheme(for: settings).style
    }

    private func color(from token: ThemeRGB, opacity: Double) -> Color {
        Color(
            red: token.red,
            green: token.green,
            blue: token.blue,
            opacity: min(1, max(0, opacity))
        )
    }

    private func contrastingTextColor(for token: ThemeRGB) -> Color {
        let luminance = (0.2126 * token.red) + (0.7152 * token.green) + (0.0722 * token.blue)
        return luminance > 0.62 ? .black.opacity(0.90) : .white
    }

    private func dimmableColor(baseColor: Color, factor: Double) -> Color {
        // Dim or lighten based on main text color
        let r = settings.fontRed
        let g = settings.fontGreen
        let b = settings.fontBlue
        let luminance = (0.2126 * r) + (0.7152 * g) + (0.0722 * b)

        if luminance > 0.5 {
            // Light text: dim towards black
            return Color(red: r * factor, green: g * factor, blue: b * factor, opacity: settings.fontOpacity)
        } else {
            // Dark text: lighten towards white
            return Color(red: r + (1.0 - r) * (1.0 - factor), green: g + (1.0 - g) * (1.0 - factor), blue: b + (1.0 - b) * (1.0 - factor), opacity: settings.fontOpacity)
        }
    }
}
