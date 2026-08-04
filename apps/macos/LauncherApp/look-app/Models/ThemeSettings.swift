import AppKit
import Foundation

enum LauncherBlurMaterial: String, CaseIterable, Codable, Identifiable {
    case hudWindow
    case sidebar
    case menu
    case underWindowBackground
    case liquidGlass

    var id: String { rawValue }

    var title: String {
        switch self {
        case .hudWindow: return "High Contrast"
        case .sidebar: return "Soft"
        case .menu: return "Balanced"
        case .underWindowBackground: return "Subtle"
        case .liquidGlass: return "Liquid Glass"
        }
    }

    var detail: String {
        switch self {
        case .hudWindow: return "Darkest and most readable"
        case .sidebar: return "Light and gentle blur"
        case .menu: return "Neutral default look"
        case .underWindowBackground: return "Most transparent feel"
        case .liquidGlass: return "Refracts the desktop behind the window"
        }
    }

    /// Liquid Glass does not render through one; it names what it degrades to
    /// on macOS 15. See `ThemedBackdrop`.
    var material: NSVisualEffectView.Material {
        switch self {
        case .hudWindow: return .hudWindow
        case .sidebar: return .sidebar
        case .menu: return .menu
        case .underWindowBackground: return .underWindowBackground
        case .liquidGlass: return .hudWindow
        }
    }

    var blurOpacityScale: Double {
        switch self {
        case .hudWindow: return 1.12
        case .sidebar: return 0.86
        case .menu: return 1.0
        case .underWindowBackground: return 0.72
        case .liquidGlass: return 1.0
        }
    }

    /// Glass carries its own depth, so the tint sits lighter on it: anything
    /// near the other materials' weight cancels the refraction.
    var tintOpacityScale: Double {
        switch self {
        case .hudWindow: return 1.16
        case .sidebar: return 0.84
        case .menu: return 1.0
        case .underWindowBackground: return 0.68
        case .liquidGlass: return 0.42
        }
    }

    /// False where the material needs an OS newer than the one running.
    var isSupported: Bool {
        switch self {
        case .liquidGlass:
            if #available(macOS 26.0, *) {
                return true
            }
            return false
        case .hudWindow, .sidebar, .menu, .underWindowBackground:
            return true
        }
    }

    /// True when this renders as glass, which has no blur to thin.
    var rendersGlass: Bool {
        self == .liquidGlass && isSupported
    }

    /// Title in Settings. An unsupported value can still be the current
    /// selection, so it says why it is inert rather than looking broken.
    var pickerTitle: String {
        isSupported ? title : "\(title) \(AppConstants.ThemeUI.unsupportedSuffix)"
    }

    /// Offered in Settings on this machine, plus `current` when that is a value
    /// this OS cannot render (a config written on a newer machine). Keeping it
    /// in the list is deliberate: a `Picker` whose selection matches no tag
    /// renders blank, and rewriting the value here would destroy the user's
    /// setting the next time they open the same config on a newer machine.
    static func options(including current: LauncherBlurMaterial) -> [LauncherBlurMaterial] {
        var options = allCases.filter(\.isSupported)
        if !options.contains(current) {
            options.append(current)
        }
        return options
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

/// On/off state for the in-search-bar running-apps row, stored as a string in
/// `~/.look.config` under `running_apps_placement`. The setting is now a simple
/// toggle (`.none` = off, `.right` = on); the legacy `.top`/`.bottom` cases are
/// retained only so old config files still decode - they are normalized to
/// `.right` ("on") on load. See ThemeStore's config parser.
enum RunningAppsPlacement: String, CaseIterable, Codable, Identifiable {
    case none
    case top
    case right
    case bottom

    var id: String { rawValue }
}

/// Which AI backend powers query understanding. On-device Apple Intelligence is
/// the only option today; cloud providers can be added as new cases without
/// touching the rest of the app. Persisted in `~/.look.config` as `ai_provider`.
enum AIProviderKind: String, CaseIterable, Codable, Identifiable {
    case appleIntelligence
    case ollama

    var id: String { rawValue }

    var title: String {
        switch self {
        case .appleIntelligence: return "Apple Intelligence (on-device)"
        case .ollama: return "Ollama (local)"
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

    /// Preset shown in the Settings picker. Custom by default because the values
    /// above are no preset's; `ThemeStore` re-detects it whenever config is read.
    var uiTheme: BuiltinThemePreset = .custom

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

    var runningAppsPlacement: RunningAppsPlacement = .right

    /// i3-style inner gap (in points) inserted between the three home-screen panes
    /// - the top row (search bar + running apps), the results list and the preview.
    /// `0` keeps the classic flat layout with hairline dividers; any value > 0 turns
    /// each pane into its own rounded card separated by empty space. Persisted in
    /// `~/.look.config` under `inner_gap`.
    var innerGap: Double = 0

    /// Whether Apple Intelligence / AI-assisted features are enabled. Defaults to
    /// on; users can opt out via Settings → Appearance. Persisted in
    /// `~/.look.config` under `ai_enabled`.
    var aiEnabled: Bool = true

    /// Which AI backend powers query understanding when `aiEnabled` is on.
    var aiProvider: AIProviderKind = .appleIntelligence

    /// Ollama daemon endpoint, used when `aiProvider` is `.ollama`. Persisted in
    /// `~/.look.config` under `ollama_host`.
    var ollamaHost: String = "http://localhost:11434"

    /// Ollama model tag, used when `aiProvider` is `.ollama`. Persisted in
    /// `~/.look.config` under `ollama_model`.
    var ollamaModel: String = "llama3.1"

    /// Whether the empty-state super actions launchpad is shown. Off hides the
    /// strip and makes its ⌘-mnemonics inert. Persisted in `~/.look.config`
    /// under `super_actions_enabled`.
    var superActionsEnabled: Bool = true

    static let `default` = ThemeSettings()
}
