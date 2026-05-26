import Foundation

enum TokyoNightTheme {
    static let style = BuiltinThemeStyle(
        themeName: "tokyoNight",
        tintRed: 0.05,
        tintGreen: 0.08,
        tintBlue: 0.16,
        tintOpacity: 0.58,
        blurMaterial: .hudWindow,
        blurOpacity: 0.95,
        fontRed: 0.84,
        fontGreen: 0.87,
        fontBlue: 0.96,
        fontOpacity: 0.98,
        borderRed: 0.45,
        borderGreen: 0.54,
        borderBlue: 0.86,
        borderOpacity: 0.24,
        textSecondary: ThemeRGB(red: 0.74, green: 0.80, blue: 0.90),
        textMuted: ThemeRGB(red: 0.56, green: 0.64, blue: 0.78),
        panelFill: ThemeRGB(red: 0.06, green: 0.09, blue: 0.18),
        panelFillOpacity: 0.38,
        controlFill: ThemeRGB(red: 0.14, green: 0.17, blue: 0.28),
        controlFillOpacity: 0.34,
        divider: ThemeRGB(red: 0.36, green: 0.42, blue: 0.60),
        dividerOpacity: 0.28,
        selectionFill: ThemeRGB(red: 0.38, green: 0.47, blue: 0.72),
        selectionFillOpacity: 0.28,
        accent: ThemeRGB(red: 0.52, green: 0.72, blue: 0.98),
        onAccent: ThemeRGB(red: 0.06, green: 0.09, blue: 0.18),
        success: ThemeRGB(red: 0.68, green: 0.91, blue: 0.65),
        warning: ThemeRGB(red: 0.96, green: 0.88, blue: 0.68),
        danger: ThemeRGB(red: 0.94, green: 0.50, blue: 0.58)
    )
}
