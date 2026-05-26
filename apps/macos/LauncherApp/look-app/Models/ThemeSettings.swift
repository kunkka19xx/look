import AppKit
import Foundation

enum LauncherBlurMaterial: String, CaseIterable, Codable, Identifiable {
    case hudWindow
    case sidebar
    case menu
    case underWindowBackground

    var id: String { rawValue }

    var title: String {
        switch self {
        case .hudWindow: return "High Contrast"
        case .sidebar: return "Soft"
        case .menu: return "Balanced"
        case .underWindowBackground: return "Subtle"
        }
    }

    var detail: String {
        switch self {
        case .hudWindow: return "Darkest and most readable"
        case .sidebar: return "Light and gentle blur"
        case .menu: return "Neutral default look"
        case .underWindowBackground: return "Most transparent feel"
        }
    }

    var material: NSVisualEffectView.Material {
        switch self {
        case .hudWindow: return .hudWindow
        case .sidebar: return .sidebar
        case .menu: return .menu
        case .underWindowBackground: return .underWindowBackground
        }
    }

    var blurOpacityScale: Double {
        switch self {
        case .hudWindow: return 1.12
        case .sidebar: return 0.86
        case .menu: return 1.0
        case .underWindowBackground: return 0.72
        }
    }

    var tintOpacityScale: Double {
        switch self {
        case .hudWindow: return 1.16
        case .sidebar: return 0.84
        case .menu: return 1.0
        case .underWindowBackground: return 0.68
        }
    }
}

enum BackgroundImageMode: String, CaseIterable, Codable, Identifiable {
    case fit
    case fill
    case stretch
    case tile

    var id: String { rawValue }

    var title: String {
        switch self {
        case .fit: return "Center"
        case .fill: return "Fill"
        case .stretch: return "Stretch"
        case .tile: return "Duplicate"
        }
    }

    var detail: String {
        switch self {
        case .fit: return "Keep full image visible"
        case .fill: return "Fill area and crop edges"
        case .stretch: return "Stretch to full bounds"
        case .tile: return "Repeat image pattern"
        }
    }
}

enum BackendLogLevel: String, CaseIterable, Codable, Identifiable {
    case error
    case info
    case debug

    var id: String { rawValue }

    var title: String {
        switch self {
        case .error: return "Error"
        case .info: return "Info"
        case .debug: return "Debug"
        }
    }
}

struct ThemeSettings: Codable, Equatable {
    var tintRed: Double = 0.08
    var tintGreen: Double = 0.10
    var tintBlue: Double = 0.12
    var tintOpacity: Double = 0.55
    var blurMaterial: LauncherBlurMaterial = .hudWindow
    var blurOpacity: Double = 0.95
    var fontName: String = "SF Pro Text"
    var fontSize: Double = 14
    var fontRed: Double = 0.96
    var fontGreen: Double = 0.96
    var fontBlue: Double = 0.98
    var fontOpacity: Double = 0.96
    var borderThickness: Double = 1.0
    var borderRed: Double = 1.0
    var borderGreen: Double = 1.0
    var borderBlue: Double = 1.0
    var borderOpacity: Double = 0.12

    var themeName: String = ""
    var uiTheme: BuiltinThemePreset = .catppuccin

    // Background image
    var backgroundImagePath: String?
    var backgroundImageBookmark: Data?
    var backgroundImageMode: BackgroundImageMode = .fill
    var backgroundImageOpacity: Double = 0.35
    var backgroundImageBlur: Double = 8

    // Settings
    var settingsBlurMultiplier: Double = 0.5

    var fileScanDepth: Int = 4
    var fileScanLimit: Int = 4000
    var lazyIndexingEnabled: Bool = true
    var backendLogLevel: BackendLogLevel = .error
    var launchAtLogin: Bool = true

    static let `default` = ThemeSettings()
}
