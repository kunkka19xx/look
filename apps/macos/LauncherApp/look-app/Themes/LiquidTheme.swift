import Foundation

/// Liquid: the glass surface as a theme of its own.
///
/// The palette is built around the material rather than over it. Every fill is
/// far more transparent than the classic presets use, because glass is a lens
/// and an opaque panel sitting on it reads as a sticker rather than depth. The
/// tint is deliberately light too: `LauncherBlurMaterial.liquidGlass` hands it
/// to `NSGlassEffectView.tintColor`, where a heavy value would cancel the
/// refraction it is supposed to colour.
///
/// Text stays near-white at full opacity. Glass runs lower contrast than
/// `hudWindow`, and the results list is dense monospaced content, so the
/// palette spends its contrast budget on type rather than on chrome.
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
        // A hairline, not a frame: glass already separates itself from the
        // desktop by refracting it, so a strong border double-states the edge.
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
        dividerOpacity: 0.12,
        // The selection pill is the one place that earns real weight: it has to
        // stay findable against a moving desktop showing through the glass.
        selectionFill: ThemeRGB(red: 0.42, green: 0.62, blue: 1.0),
        selectionFillOpacity: 0.34,
        accent: ThemeRGB(red: 0.44, green: 0.72, blue: 1.0),
        onAccent: ThemeRGB(red: 0.04, green: 0.07, blue: 0.13),
        success: ThemeRGB(red: 0.40, green: 0.86, blue: 0.66),
        warning: ThemeRGB(red: 1.0, green: 0.78, blue: 0.42),
        danger: ThemeRGB(red: 1.0, green: 0.46, blue: 0.52)
    )
}
