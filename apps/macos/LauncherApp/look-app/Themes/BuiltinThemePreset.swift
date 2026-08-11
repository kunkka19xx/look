import Foundation

enum BuiltinThemePreset: String, CaseIterable, Identifiable, Codable {
    case custom
    case catppuccin
    case tokyoNight
    case rosePine
    case gruvbox
    case dracula
    case kanagawa
    case kindle
    case liquid

    var id: String { rawValue }

    /// Liquid needs the macOS 26 glass effect.
    var isSupported: Bool {
        self != .liquid || LauncherBlurMaterial.liquidGlass.isSupported
    }

    /// Offered in Settings on this machine, plus `current` when that is a preset
    /// this OS cannot render. See `LauncherBlurMaterial.options(including:)` for
    /// why the unsupported value stays in the list rather than being rewritten.
    static func options(including current: BuiltinThemePreset) -> [BuiltinThemePreset] {
        var options = allCases.filter(\.isSupported)
        if !options.contains(current) {
            options.append(current)
        }
        return options
    }

    var title: String {
        switch self {
        case .custom: return "Custom"
        case .catppuccin: return "Catppuccin"
        case .tokyoNight: return "Tokyo Night"
        case .rosePine: return "Rose Pine"
        case .gruvbox: return "Gruvbox"
        case .dracula: return "Dracula"
        case .kanagawa: return "Kanagawa"
        case .kindle: return "Kindle"
        case .liquid: return "Liquid"
        }
    }

    /// Title in Settings, flagging a preset this OS cannot render.
    var pickerTitle: String {
        isSupported ? title : "\(title) \(AppConstants.ThemeUI.unsupportedSuffix)"
    }
}
