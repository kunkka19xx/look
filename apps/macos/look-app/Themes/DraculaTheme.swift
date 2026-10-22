import Foundation

enum DraculaTheme {
    static let style = BuiltinThemeStyle(
        themeName: "dracula",
        tintRed: 0.09,
        tintGreen: 0.08,
        tintBlue: 0.15,
        tintOpacity: 0.58,
        blurMaterial: .menu,
        blurOpacity: 0.94,
        fontRed: 0.97,
        fontGreen: 0.97,
        fontBlue: 0.98,
        fontOpacity: 0.98,
        borderRed: 0.74,
        borderGreen: 0.55,
        borderBlue: 0.89,
        borderOpacity: 0.24,
        textSecondary: ThemeRGB(red: 0.92, green: 0.87, blue: 0.98),
        textMuted: ThemeRGB(red: 0.77, green: 0.74, blue: 0.85),
        panelFill: ThemeRGB(red: 0.11, green: 0.10, blue: 0.18),
        panelFillOpacity: 0.38,
        controlFill: ThemeRGB(red: 0.21, green: 0.20, blue: 0.30),
        controlFillOpacity: 0.34,
        divider: ThemeRGB(red: 0.52, green: 0.42, blue: 0.66),
        dividerOpacity: 0.28,
        selectionFill: ThemeRGB(red: 0.62, green: 0.52, blue: 0.79),
        selectionFillOpacity: 0.28,
        accent: ThemeRGB(red: 0.64, green: 0.75, blue: 0.98),
        onAccent: ThemeRGB(red: 0.11, green: 0.10, blue: 0.18),
        success: ThemeRGB(red: 0.68, green: 0.91, blue: 0.65),
        warning: ThemeRGB(red: 0.99, green: 0.91, blue: 0.71),
        danger: ThemeRGB(red: 0.98, green: 0.55, blue: 0.61)
    )
}
