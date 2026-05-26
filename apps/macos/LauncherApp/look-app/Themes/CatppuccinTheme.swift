import Foundation

enum CatppuccinTheme {
    static let style = BuiltinThemeStyle(
        themeName: "catppuccin",
        tintRed: 0.11,
        tintGreen: 0.11,
        tintBlue: 0.18,
        tintOpacity: 0.58,
        blurMaterial: .hudWindow,
        blurOpacity: 0.94,
        fontRed: 0.95,
        fontGreen: 0.94,
        fontBlue: 0.98,
        fontOpacity: 0.97,
        borderRed: 0.80,
        borderGreen: 0.75,
        borderBlue: 0.93,
        borderOpacity: 0.20,
        textSecondary: ThemeRGB(red: 0.80, green: 0.84, blue: 0.96),
        textMuted: ThemeRGB(red: 0.67, green: 0.70, blue: 0.78),
        panelFill: ThemeRGB(red: 0.12, green: 0.11, blue: 0.18),
        panelFillOpacity: 0.34,
        controlFill: ThemeRGB(red: 0.19, green: 0.20, blue: 0.27),
        controlFillOpacity: 0.34,
        divider: ThemeRGB(red: 0.50, green: 0.52, blue: 0.62),
        dividerOpacity: 0.32,
        selectionFill: ThemeRGB(red: 0.58, green: 0.60, blue: 0.70),
        selectionFillOpacity: 0.28,
        accent: ThemeRGB(red: 0.54, green: 0.71, blue: 0.98),
        onAccent: ThemeRGB(red: 0.11, green: 0.11, blue: 0.18),
        success: ThemeRGB(red: 0.65, green: 0.89, blue: 0.63),
        warning: ThemeRGB(red: 0.98, green: 0.89, blue: 0.69),
        danger: ThemeRGB(red: 0.95, green: 0.55, blue: 0.66)
    )
}
