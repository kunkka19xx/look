import Foundation

extension BuiltinThemePreset {
    var style: BuiltinThemeStyle? {
        switch self {
        case .custom:
            return nil
        case .catppuccin:
            return CatppuccinTheme.style
        case .tokyoNight:
            return TokyoNightTheme.style
        case .rosePine:
            return RosePineTheme.style
        case .gruvbox:
            return GruvboxTheme.style
        case .dracula:
            return DraculaTheme.style
        case .kanagawa:
            return KanagawaTheme.style
        case .kindle:
            return KindleTheme.style
        case .liquid:
            return LiquidTheme.style
        }
    }
}

extension BuiltinThemePreset {
    /// Resolves the `ui_theme` config value in any casing. Matches on the
    /// style's `themeName`, not the enum raw value, so `tokyoNight` resolves.
    static func preset(forThemeName name: String) -> BuiltinThemePreset? {
        let needle = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !needle.isEmpty else {
            return nil
        }
        return allCases.first { $0.style?.themeName.lowercased() == needle }
    }
}
