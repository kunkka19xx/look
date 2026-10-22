import Foundation

enum KanagawaTheme {
    static let style = BuiltinThemeStyle(
        themeName: "kanagawa",
        tintRed: 0.09,
        tintGreen: 0.10,
        tintBlue: 0.12,
        tintOpacity: 0.58,
        blurMaterial: .hudWindow,
        blurOpacity: 0.95,
        fontRed: 0.87,
        fontGreen: 0.86,
        fontBlue: 0.79,
        fontOpacity: 0.98,
        borderRed: 0.58,
        borderGreen: 0.52,
        borderBlue: 0.42,
        borderOpacity: 0.22,
        textSecondary: ThemeRGB(red: 0.80, green: 0.78, blue: 0.66),
        textMuted: ThemeRGB(red: 0.66, green: 0.63, blue: 0.50),
        panelFill: ThemeRGB(red: 0.10, green: 0.12, blue: 0.14),
        panelFillOpacity: 0.38,
        controlFill: ThemeRGB(red: 0.18, green: 0.20, blue: 0.24),
        controlFillOpacity: 0.34,
        divider: ThemeRGB(red: 0.42, green: 0.40, blue: 0.34),
        dividerOpacity: 0.28,
        selectionFill: ThemeRGB(red: 0.50, green: 0.48, blue: 0.38),
        selectionFillOpacity: 0.28,
        accent: ThemeRGB(red: 0.46, green: 0.65, blue: 0.82),
        onAccent: ThemeRGB(red: 0.10, green: 0.12, blue: 0.14),
        success: ThemeRGB(red: 0.66, green: 0.88, blue: 0.62),
        warning: ThemeRGB(red: 0.96, green: 0.85, blue: 0.66),
        danger: ThemeRGB(red: 0.91, green: 0.48, blue: 0.52)
    )
}
