import Foundation

enum BuiltinThemePreset: String, CaseIterable, Identifiable, Codable {
    case custom
    case catppuccin
    case tokyoNight
    case rosePine
    case gruvbox
    case dracula
    case kanagawa

    var id: String { rawValue }

    var title: String {
        switch self {
        case .custom: return "Custom"
        case .catppuccin: return "Catppuccin"
        case .tokyoNight: return "Tokyo Night"
        case .rosePine: return "Rose Pine"
        case .gruvbox: return "Gruvbox"
        case .dracula: return "Dracula"
        case .kanagawa: return "Kanagawa"
        }
    }
}
