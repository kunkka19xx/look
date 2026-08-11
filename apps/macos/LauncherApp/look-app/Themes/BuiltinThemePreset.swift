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

    /// Presets offered in Settings on this machine. Liquid renders through the
    /// macOS 26 glass effect, so it is omitted where that does not exist rather
    /// than shipping a preset that silently looks like something else.
    static var selectable: [BuiltinThemePreset] {
        allCases.filter { $0 != .liquid || LauncherBlurMaterial.liquidGlass.isSupported }
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
}
