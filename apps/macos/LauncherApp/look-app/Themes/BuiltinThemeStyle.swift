import Foundation

struct ThemeRGB {
    let red: Double
    let green: Double
    let blue: Double
}

struct BuiltinThemeStyle {
    let themeName: String
    let tintRed: Double
    let tintGreen: Double
    let tintBlue: Double
    let tintOpacity: Double
    let blurMaterial: LauncherBlurMaterial
    let blurOpacity: Double
    let fontRed: Double
    let fontGreen: Double
    let fontBlue: Double
    let fontOpacity: Double
    let borderRed: Double
    let borderGreen: Double
    let borderBlue: Double
    let borderOpacity: Double

    // Semantic color adjustments (relative to main text color when theme applied)
    // -1.0 = black, 0 = same, +1.0 = white
    let textSecondaryAdjust: Double = -0.18
    let textMutedAdjust: Double = -0.35
    let panelFillAdjust: Double = -0.85
    let controlFillAdjust: Double = -0.80
    let dividerAdjust: Double = -0.70
    let selectionFillAdjust: Double = -0.55
    let accentAdjust: Double = 0.0  // will use actual accent color

    // Absolute semantic colors (override adjustments when set)
    let textSecondary: ThemeRGB?
    let textMuted: ThemeRGB?
    let panelFill: ThemeRGB?
    let panelFillOpacity: Double
    let controlFill: ThemeRGB?
    let controlFillOpacity: Double
    let divider: ThemeRGB?
    let dividerOpacity: Double
    let selectionFill: ThemeRGB?
    let selectionFillOpacity: Double
    let accent: ThemeRGB?
    let onAccent: ThemeRGB?
    let success: ThemeRGB?
    let warning: ThemeRGB?
    let danger: ThemeRGB?

init(
        themeName: String = "",
        tintRed: Double = 0.08,
        tintGreen: Double = 0.10,
        tintBlue: Double = 0.12,
        tintOpacity: Double = 0.55,
        blurMaterial: LauncherBlurMaterial = .hudWindow,
        blurOpacity: Double = 0.95,
        fontRed: Double = 0.96,
        fontGreen: Double = 0.96,
        fontBlue: Double = 0.98,
        fontOpacity: Double = 0.96,
        borderRed: Double = 1.0,
        borderGreen: Double = 1.0,
        borderBlue: Double = 1.0,
        borderOpacity: Double = 0.12,
        textSecondary: ThemeRGB? = nil,
        textMuted: ThemeRGB? = nil,
        panelFill: ThemeRGB? = nil,
        panelFillOpacity: Double = 0.30,
        controlFill: ThemeRGB? = nil,
        controlFillOpacity: Double = 0.30,
        divider: ThemeRGB? = nil,
        dividerOpacity: Double = 0.20,
        selectionFill: ThemeRGB? = nil,
        selectionFillOpacity: Double = 0.25,
        accent: ThemeRGB? = nil,
        onAccent: ThemeRGB? = nil,
        success: ThemeRGB? = nil,
        warning: ThemeRGB? = nil,
        danger: ThemeRGB? = nil
    ) {
        self.themeName = themeName
        self.tintRed = tintRed
        self.tintGreen = tintGreen
        self.tintBlue = tintBlue
        self.tintOpacity = tintOpacity
        self.blurMaterial = blurMaterial
        self.blurOpacity = blurOpacity
        self.fontRed = fontRed
        self.fontGreen = fontGreen
        self.fontBlue = fontBlue
        self.fontOpacity = fontOpacity
        self.borderRed = borderRed
        self.borderGreen = borderGreen
        self.borderBlue = borderBlue
        self.borderOpacity = borderOpacity
        self.textSecondary = textSecondary
        self.textMuted = textMuted
        self.panelFill = panelFill
        self.panelFillOpacity = panelFillOpacity
        self.controlFill = controlFill
        self.controlFillOpacity = controlFillOpacity
        self.divider = divider
        self.dividerOpacity = dividerOpacity
        self.selectionFill = selectionFill
        self.selectionFillOpacity = selectionFillOpacity
        self.accent = accent
        self.onAccent = onAccent
        self.success = success
        self.warning = warning
        self.danger = danger
    }

    func apply(to settings: inout ThemeSettings) {
        // User-configurable settings only
        settings.tintRed = tintRed
        settings.tintGreen = tintGreen
        settings.tintBlue = tintBlue
        settings.tintOpacity = tintOpacity
        settings.blurMaterial = blurMaterial
        settings.blurOpacity = blurOpacity
        settings.fontRed = fontRed
        settings.fontGreen = fontGreen
        settings.fontBlue = fontBlue
        settings.fontOpacity = fontOpacity
        settings.borderRed = borderRed
        settings.borderGreen = borderGreen
        settings.borderBlue = borderBlue
        settings.borderOpacity = borderOpacity
        settings.themeName = self.themeName
    }

    func matches(_ settings: ThemeSettings, tolerance: Double = 0.01) -> Bool {
        abs(settings.tintRed - tintRed) <= tolerance
            && abs(settings.tintGreen - tintGreen) <= tolerance
            && abs(settings.tintBlue - tintBlue) <= tolerance
            && abs(settings.tintOpacity - tintOpacity) <= tolerance
            && settings.blurMaterial == blurMaterial
            && abs(settings.blurOpacity - blurOpacity) <= tolerance
            && abs(settings.fontRed - fontRed) <= tolerance
            && abs(settings.fontGreen - fontGreen) <= tolerance
            && abs(settings.fontBlue - fontBlue) <= tolerance
            && abs(settings.fontOpacity - fontOpacity) <= tolerance
            && abs(settings.borderRed - borderRed) <= tolerance
            && abs(settings.borderGreen - borderGreen) <= tolerance
            && abs(settings.borderBlue - borderBlue) <= tolerance
            && abs(settings.borderOpacity - borderOpacity) <= tolerance
    }
}
