import SwiftUI

extension ThemeStore {
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
        Color(
            .sRGB,
            red: 0.18 + settings.tintRed * 0.25,
            green: 0.18 + settings.tintGreen * 0.25,
            blue: 0.20 + settings.tintBlue * 0.25,
            opacity: 1.0
        )
    }

    func commandModePanelColor() -> Color {
        Color(
            .sRGB,
            red: 0.13 + settings.tintRed * 0.25,
            green: 0.13 + settings.tintGreen * 0.25,
            blue: 0.15 + settings.tintBlue * 0.25,
            opacity: 1.0
        )
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
            return
        }
        style.apply(to: &settings)
    }

    func detectBuiltinTheme(for settings: ThemeSettings) -> BuiltinThemePreset {
        if !settings.themeName.isEmpty {
            for preset in BuiltinThemePreset.allCases where preset != .custom {
                if preset.style?.themeName.lowercased() == settings.themeName.lowercased() {
                    return preset
                }
            }
        }
        for preset in BuiltinThemePreset.allCases where preset != .custom {
            if let style = preset.style, style.matches(settings) {
                return preset
            }
        }
        return .custom
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
