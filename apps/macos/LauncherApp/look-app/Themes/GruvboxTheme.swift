import Foundation

enum GruvboxTheme {
    static let style = BuiltinThemeStyle(
        themeName: "gruvbox",
        tintRed: 0.13,
        tintGreen: 0.10,
        tintBlue: 0.07,
        tintOpacity: 0.60,
        blurMaterial: .hudWindow,
        blurOpacity: 0.95,
        fontRed: 0.93,
        fontGreen: 0.89,
        fontBlue: 0.79,
        fontOpacity: 0.98,
        borderRed: 0.84,
        borderGreen: 0.54,
        borderBlue: 0.26,
        borderOpacity: 0.24,
        textSecondary: ThemeRGB(red: 0.87, green: 0.80, blue: 0.64),
        textMuted: ThemeRGB(red: 0.72, green: 0.64, blue: 0.48),
        panelFill: ThemeRGB(red: 0.14, green: 0.11, blue: 0.09),
        panelFillOpacity: 0.38,
        controlFill: ThemeRGB(red: 0.21, green: 0.17, blue: 0.13),
        controlFillOpacity: 0.34,
        divider: ThemeRGB(red: 0.52, green: 0.40, blue: 0.28),
        dividerOpacity: 0.28,
        selectionFill: ThemeRGB(red: 0.74, green: 0.54, blue: 0.26),
        selectionFillOpacity: 0.28,
        accent: ThemeRGB(red: 0.86, green: 0.72, blue: 0.40),
        onAccent: ThemeRGB(red: 0.14, green: 0.11, blue: 0.09),
        success: ThemeRGB(red: 0.71, green: 0.91, blue: 0.64),
        warning: ThemeRGB(red: 0.98, green: 0.89, blue: 0.68),
        danger: ThemeRGB(red: 0.96, green: 0.52, blue: 0.56)
    )
}
