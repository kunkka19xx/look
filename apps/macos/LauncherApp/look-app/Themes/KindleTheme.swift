import Foundation

/// Kindle Paperwhite: warm paper, near-black ink, greyscale semantics, and a
/// high tint opacity so it reads matte rather than frosted. Charter is the
/// stand-in for Bookerly, which Amazon ships only on its own readers.
enum KindleTheme {
    static let style = BuiltinThemeStyle(
        themeName: "kindle",
        appearance: .light,
        tintRed: 0.97,
        tintGreen: 0.95,
        tintBlue: 0.90,
        tintOpacity: 0.93,
        blurMaterial: .menu,
        blurOpacity: 0.98,
        fontName: "Charter",
        fontRed: 0.13,
        fontGreen: 0.12,
        fontBlue: 0.10,
        fontOpacity: 1.0,
        borderRed: 0.42,
        borderGreen: 0.38,
        borderBlue: 0.32,
        borderOpacity: 0.26,
        textSecondary: ThemeRGB(red: 0.24, green: 0.22, blue: 0.19),
        textMuted: ThemeRGB(red: 0.30, green: 0.28, blue: 0.24),
        panelFill: ThemeRGB(red: 1.0, green: 0.99, blue: 0.96),
        panelFillOpacity: 0.55,
        controlFill: ThemeRGB(red: 0.85, green: 0.82, blue: 0.76),
        controlFillOpacity: 0.55,
        divider: ThemeRGB(red: 0.40, green: 0.37, blue: 0.32),
        dividerOpacity: 0.30,
        selectionFill: ThemeRGB(red: 0.24, green: 0.22, blue: 0.19),
        selectionFillOpacity: 0.16,
        accent: ThemeRGB(red: 0.18, green: 0.17, blue: 0.15),
        onAccent: ThemeRGB(red: 0.97, green: 0.95, blue: 0.90),
        success: ThemeRGB(red: 0.16, green: 0.42, blue: 0.22),
        warning: ThemeRGB(red: 0.52, green: 0.35, blue: 0.03),
        danger: ThemeRGB(red: 0.62, green: 0.15, blue: 0.13)
    )
}
