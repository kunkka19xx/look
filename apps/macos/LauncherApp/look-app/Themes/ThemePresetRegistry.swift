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
        }
    }
}
