import AppKit
import Foundation

struct ThemeRGB {
    let red: Double
    let green: Double
    let blue: Double
}

/// Surface a preset paints on. Drives the pinned window/blur appearance and the
/// opaque command-mode colors, neither of which can be derived from the tokens.
enum ThemeAppearance {
    case dark
    case light

    var nsAppearanceName: NSAppearance.Name {
        switch self {
        case .dark: return .darkAqua
        case .light: return .aqua
        }
    }
}

/// How a preset renders its surfaces, as opposed to what colour they are. The
/// second non-token axis a style carries, alongside `appearance`.
enum ThemeSurface {
    /// Blur plus tint.
    case classic
    case liquid

    /// Glass reads as a lens; a tight corner makes it a clipped rectangle.
    static let liquidRadiusScale: CGFloat = 1.5

    var cornerRadiusScale: CGFloat {
        switch self {
        case .classic: return 1
        case .liquid: return Self.liquidRadiusScale
        }
    }
}

struct BuiltinThemeStyle {
    let themeName: String
    let appearance: ThemeAppearance
    /// Not written into `ThemeSettings`: like `appearance`, read back off the
    /// resolved style rather than stored per-key.
    let surface: ThemeSurface
    let tintRed: Double
    let tintGreen: Double
    let tintBlue: Double
    let tintOpacity: Double
    let blurMaterial: LauncherBlurMaterial
    let blurOpacity: Double
    /// nil keeps the app default. An unavailable family falls back to the
    /// system font via `ThemeStore.uiFont`.
    let fontName: String?
    let fontRed: Double
    let fontGreen: Double
    let fontBlue: Double
    let fontOpacity: Double
    let borderRed: Double
    let borderGreen: Double
    let borderBlue: Double
    let borderOpacity: Double

    // Semantic colors. Left nil, `ThemeStore` derives the token from the main
    // text color instead.
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
        appearance: ThemeAppearance = .dark,
        surface: ThemeSurface = .classic,
        tintRed: Double = 0.08,
        tintGreen: Double = 0.10,
        tintBlue: Double = 0.12,
        tintOpacity: Double = 0.55,
        blurMaterial: LauncherBlurMaterial = .hudWindow,
        blurOpacity: Double = 0.95,
        fontName: String? = nil,
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
        self.appearance = appearance
        self.surface = surface
        self.tintRed = tintRed
        self.tintGreen = tintGreen
        self.tintBlue = tintBlue
        self.tintOpacity = tintOpacity
        self.blurMaterial = blurMaterial
        self.blurOpacity = blurOpacity
        self.fontName = fontName
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
        // Only a preset that names a font imposes one. Liquid passes nil, and
        // forcing the default there reset the user's typeface every load, since
        // `matches` now keeps `ui_theme` in the config across a font change.
        if let fontName {
            settings.fontName = fontName
        }
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

    /// Deliberately ignores `fontName`. A preset still SETS one when applied
    /// (see `apply`), but the typeface is not what makes a theme that theme:
    /// picking a different font would otherwise drop Liquid Glass to Custom and
    /// take `ui_theme` out of the config with it. Font *size* has never been
    /// compared here, so this only makes the two agree. Text colour stays in,
    /// since that is part of the look.
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
