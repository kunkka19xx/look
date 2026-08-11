import Foundation

/// Liquid: the glass surface as a theme of its own.
///
/// Fills are far more transparent than the classic presets use: glass is a lens,
/// and an opaque panel on it reads as a sticker rather than depth. Text keeps
/// full contrast, since glass is dimmer than `hudWindow` and the results list is
/// dense monospaced content.
enum LiquidTheme {
    static let style = BuiltinThemeStyle(
        themeName: "liquid",
        appearance: .dark,
        surface: .liquid,
        tintRed: 0.10,
        tintGreen: 0.13,
        tintBlue: 0.20,
        tintOpacity: 0.35,
        blurMaterial: .liquidGlass,
        blurOpacity: 1.0,
        fontName: nil,
        fontRed: 0.98,
        fontGreen: 0.98,
        fontBlue: 1.0,
        fontOpacity: 1.0,
        // A hairline: refraction already states the edge.
        borderRed: 1.0,
        borderGreen: 1.0,
        borderBlue: 1.0,
        borderOpacity: 0.10,
        textSecondary: ThemeRGB(red: 0.86, green: 0.89, blue: 0.95),
        textMuted: ThemeRGB(red: 0.72, green: 0.77, blue: 0.87),
        panelFill: ThemeRGB(red: 0.62, green: 0.72, blue: 0.92),
        panelFillOpacity: 0.10,
        controlFill: ThemeRGB(red: 0.66, green: 0.76, blue: 0.94),
        controlFillOpacity: 0.12,
        divider: ThemeRGB(red: 1.0, green: 1.0, blue: 1.0),
        dividerOpacity: 0.18,
        // Heavier than a flat theme would need: it sits over refracting glass
        // with the desktop moving behind it, where a light wash disappears.
        selectionFill: ThemeRGB(red: 0.40, green: 0.60, blue: 1.0),
        selectionFillOpacity: 0.58,
        accent: ThemeRGB(red: 0.44, green: 0.72, blue: 1.0),
        onAccent: ThemeRGB(red: 0.04, green: 0.07, blue: 0.13),
        success: ThemeRGB(red: 0.40, green: 0.86, blue: 0.66),
        warning: ThemeRGB(red: 1.0, green: 0.78, blue: 0.42),
        danger: ThemeRGB(red: 1.0, green: 0.46, blue: 0.52)
    )
}
