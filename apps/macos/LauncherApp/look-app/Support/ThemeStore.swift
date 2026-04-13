import Foundation
import Combine
import SwiftUI
import AppKit
import ServiceManagement

final class ThemeStore: ObservableObject {
    @Published private(set) var backgroundImageURL: URL?
    @Published private(set) var backgroundImage: NSImage?
    @Published var uiScale: CGFloat = 1.0

    @Published var settings: ThemeSettings {
        didSet {
            save()
            if oldValue.backgroundImagePath != settings.backgroundImagePath
                || oldValue.backgroundImageBookmark != settings.backgroundImageBookmark
            {
                refreshBackgroundImageURL()
            }
        }
    }

    @Published private(set) var excludedFolderPaths: [String] = []

    private let defaultsKey = "look.theme.settings"
    private let cachedFontFamilies: [String] = NSFontManager.shared.availableFontFamilies.sorted {
        $0.localizedCaseInsensitiveCompare($1) == .orderedAscending
    }
    private var scopedBackgroundURL: URL?

    init() {
        Self.ensureDefaultConfigFileExists(at: Self.configPath())

        settings = Self.loadThemeSettings(from: UserDefaults.standard.data(forKey: defaultsKey))

        applyThemeOverridesFromConfigFile()
        _ = applyLaunchAtLoginSetting()

        refreshBackgroundImageURL()
    }

    func reset() {
        settings = .default
        applyThemeOverridesFromConfigFile()
    }

    struct ConfigReloadResult {
        let loadedTheme: String
        let warnings: [String]
        let settingsBlurMultiplier: Double?
    }

    func reloadFromConfig() -> ConfigReloadResult {
        Self.ensureDefaultConfigFileExists(at: Self.configPath())

        var warnings: [String] = []
        var loadedSettingsBlurMultiplier: Double? = nil
        let configPath = Self.configPath()

        // First, scan raw config for invalid values before applying
        if let raw = try? String(contentsOf: configPath, encoding: .utf8) {
            for line in raw.split(whereSeparator: \ .isNewline) {
                let stripped = line.trimmingCharacters(in: .whitespacesAndNewlines)
                guard stripped.firstIndex(of: "=") != nil else { continue }

                let parts = stripped.split(separator: "=", maxSplits: 1)
                guard parts.count == 2 else { continue }
                let key = String(parts[0]).trimmingCharacters(in: .whitespacesAndNewlines)
                let value = String(parts[1]).trimmingCharacters(in: .whitespacesAndNewlines)

                switch key {
                case "ui_tint_red", "ui_tint_green", "ui_tint_blue", "ui_tint_opacity", "ui_font_opacity", "ui_border_opacity":
                    if let parsed = Double(value), parsed < 0 || parsed > 1 {
                        warnings.append("\(key)=\(value) invalid (expected 0-1)")
                    }
                case "ui_font_size":
                    if let parsed = Double(value), parsed <= 0 {
                        warnings.append("\(key)=\(value) invalid (must be > 0)")
                    }
                case "file_scan_depth":
                    if let parsed = Int(value), parsed < AppConstants.FileScan.minDepth || parsed > AppConstants.FileScan.maxDepth {
                        warnings.append("\(key)=\(value) invalid (must be \(AppConstants.FileScan.minDepth)-\(AppConstants.FileScan.maxDepth))")
                    }
                case "file_scan_limit":
                    if let parsed = Int(value), parsed < AppConstants.FileScan.minLimit || parsed > AppConstants.FileScan.maxLimit {
                        warnings.append("\(key)=\(value) invalid (must be \(AppConstants.FileScan.minLimit)-\(AppConstants.FileScan.maxLimit))")
                    }
                case "ui_background_image":
                    if !value.isEmpty {
                        let url = URL(fileURLWithPath: value)
                        if !FileManager.default.fileExists(atPath: url.path) {
                            warnings.append("background image not found: \(value)")
                        }
                    }
                case "ui_theme":
                    if !value.isEmpty {
                        let themeValue = value.lowercased()
                        let validThemes = ["catppuccin", "tokyonight", "rosepine", "gruvbox", "dracula", "kanagawa"]
                        if validThemes.contains(themeValue) {
                            let preset = BuiltinThemePreset(rawValue: themeValue) ?? .custom
                            settings.uiTheme = preset
                            applyBuiltinTheme(preset)
                        } else {
                            warnings.append("theme '\(value)' not found")
                        }
                    }
                default:
                    break
                }
            }
        }

        // Save original values before parsing
        let originalTintRed = settings.tintRed
        let originalTintGreen = settings.tintGreen
        let originalTintBlue = settings.tintBlue
        let originalTintOpacity = settings.tintOpacity
        let originalFontSize = settings.fontSize

        // Apply config
        applyThemeOverridesFromConfigFile()

        // If any were invalid, reset to defaults
        if warnings.contains(where: { $0.hasPrefix("ui_tint_red") }) {
            settings.tintRed = originalTintRed
        }
        if warnings.contains(where: { $0.hasPrefix("ui_tint_green") }) {
            settings.tintGreen = originalTintGreen
        }
        if warnings.contains(where: { $0.hasPrefix("ui_tint_blue") }) {
            settings.tintBlue = originalTintBlue
        }
        if warnings.contains(where: { $0.hasPrefix("ui_tint_opacity") }) {
            settings.tintOpacity = originalTintOpacity
        }
        if warnings.contains(where: { $0.hasPrefix("ui_font_size") }) {
            settings.fontSize = originalFontSize
        }

        _ = applyLaunchAtLoginSetting()

        let resultTheme = detectBuiltinTheme(for: settings)
        let loadedBlurMultiplier = settings.settingsBlurMultiplier
        return ConfigReloadResult(loadedTheme: resultTheme.title, warnings: warnings, settingsBlurMultiplier: loadedBlurMultiplier)
    }

    func saveCurrentConfigToFile() -> Bool {
        let path = Self.configPath()
        Self.ensureDefaultConfigFileExists(at: path)

        var lines = ((try? String(contentsOf: path, encoding: .utf8)) ?? "")
            .split(omittingEmptySubsequences: false, whereSeparator: \ .isNewline)
            .map(String.init)

        if !lines.contains(where: { stripComment($0).trimmingCharacters(in: .whitespacesAndNewlines) == "# UI theme" }) {
            if !lines.isEmpty, !(lines.last?.isEmpty ?? true) {
                lines.append("")
            }
            lines.append("# UI theme")
        }

        upsertConfigLine(&lines, key: "ui_tint_red", value: String(format: "%.2f", settings.tintRed))
        upsertConfigLine(&lines, key: "ui_tint_green", value: String(format: "%.2f", settings.tintGreen))
        upsertConfigLine(&lines, key: "ui_tint_blue", value: String(format: "%.2f", settings.tintBlue))
        upsertConfigLine(&lines, key: "ui_tint_opacity", value: String(format: "%.2f", settings.tintOpacity))
        upsertConfigLine(&lines, key: "ui_blur_material", value: settings.blurMaterial.rawValue)
        upsertConfigLine(&lines, key: "ui_blur_opacity", value: String(format: "%.2f", settings.blurOpacity))
        upsertConfigLine(&lines, key: "ui_font_name", value: settings.fontName)
        upsertConfigLine(&lines, key: "ui_font_size", value: String(format: "%.0f", settings.fontSize))
        upsertConfigLine(&lines, key: "ui_font_red", value: String(format: "%.2f", settings.fontRed))
        upsertConfigLine(&lines, key: "ui_font_green", value: String(format: "%.2f", settings.fontGreen))
        upsertConfigLine(&lines, key: "ui_font_blue", value: String(format: "%.2f", settings.fontBlue))
        upsertConfigLine(&lines, key: "ui_font_opacity", value: String(format: "%.2f", settings.fontOpacity))
        upsertConfigLine(&lines, key: "ui_border_thickness", value: String(format: "%.2f", settings.borderThickness))
        upsertConfigLine(&lines, key: "ui_border_red", value: String(format: "%.2f", settings.borderRed))
        upsertConfigLine(&lines, key: "ui_border_green", value: String(format: "%.2f", settings.borderGreen))
        upsertConfigLine(&lines, key: "ui_border_blue", value: String(format: "%.2f", settings.borderBlue))
        upsertConfigLine(&lines, key: "ui_border_opacity", value: String(format: "%.2f", settings.borderOpacity))
        upsertConfigLine(&lines, key: "file_scan_depth", value: String(settings.fileScanDepth))
        upsertConfigLine(&lines, key: "file_scan_limit", value: String(settings.fileScanLimit))
        upsertConfigLine(
            &lines,
            key: "lazy_indexing_enabled",
            value: settings.lazyIndexingEnabled ? "true" : "false"
        )
        upsertConfigLine(
            &lines,
            key: "file_exclude_paths",
            value: excludedFolderPaths.map(escapeCSVToken).joined(separator: ",")
        )
        upsertConfigLine(&lines, key: "backend_log_level", value: settings.backendLogLevel.rawValue)
        upsertConfigLine(&lines, key: "launch_at_login", value: settings.launchAtLogin ? "true" : "false")

        // Background image
        if let bgPath = settings.backgroundImagePath, !bgPath.isEmpty {
            upsertConfigLine(&lines, key: "ui_background_image", value: bgPath)
            upsertConfigLine(&lines, key: "ui_background_image_mode", value: settings.backgroundImageMode.rawValue)
            upsertConfigLine(&lines, key: "ui_background_image_opacity", value: String(format: "%.2f", settings.backgroundImageOpacity))
            upsertConfigLine(&lines, key: "ui_background_image_blur", value: String(format: "%.1f", settings.backgroundImageBlur))
        } else {
            removeConfigLine(&lines, key: "ui_background_image")
            removeConfigLine(&lines, key: "ui_background_image_mode")
            removeConfigLine(&lines, key: "ui_background_image_opacity")
            removeConfigLine(&lines, key: "ui_background_image_blur")
        }

        // Settings blur multiplier
        upsertConfigLine(&lines, key: "settings_blur_multiplier", value: String(format: "%.2f", settings.settingsBlurMultiplier))

        let payload = lines.joined(separator: "\n") + "\n"
        do {
            try payload.write(to: path, atomically: true, encoding: .utf8)
            _ = applyLaunchAtLoginSetting()
            return true
        } catch {
            return false
        }
    }

    func regenerateFreshConfigFile() -> Bool {
        let path = Self.configPath()
        if FileManager.default.fileExists(atPath: path.path) {
            do {
                try FileManager.default.removeItem(at: path)
            } catch {
                return false
            }
        }

        Self.ensureDefaultConfigFileExists(at: path)
        guard FileManager.default.fileExists(atPath: path.path) else {
            return false
        }

        settings = .default
        applyThemeOverridesFromConfigFile()
        _ = applyLaunchAtLoginSetting()
        return true
    }

    func zoomIn() {
        uiScale = min(1.8, uiScale + 0.1)
    }

    func zoomOut() {
        uiScale = max(0.7, uiScale - 0.1)
    }

    func resetZoom() {
        uiScale = 1.0
    }

    func uiFont(size: CGFloat? = nil, weight: Font.Weight = .regular) -> Font {
        let baseSize = size ?? CGFloat(settings.fontSize)
        let resolvedSize = max(8, baseSize * uiScale)
        let resolvedName = settings.fontName.trimmingCharacters(in: .whitespacesAndNewlines)
        if !resolvedName.isEmpty, let fontName = resolveUsableFontName(resolvedName) {
            return .custom(fontName, size: resolvedSize).weight(weight)
        }

        return .system(size: resolvedSize, weight: weight)
    }

    func fontNameSuggestions(for input: String, limit: Int = 8) -> [String] {
        let allFonts = cachedFontFamilies
        let query = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if query.isEmpty {
            return Array(allFonts.prefix(limit))
        }

        let lowered = query.lowercased()
        var startsWithMatches = allFonts.filter { $0.lowercased().hasPrefix(lowered) }
        let containsMatches = allFonts.filter { !$0.lowercased().hasPrefix(lowered) && $0.lowercased().contains(lowered) }
        startsWithMatches.append(contentsOf: containsMatches)
        return Array(startsWithMatches.prefix(limit))
    }

    func setBackgroundImage(url: URL?) {
        guard let url else {
            settings.backgroundImagePath = nil
            settings.backgroundImageBookmark = nil
            return
        }

        let bookmark = try? url.bookmarkData(
            options: .withSecurityScope,
            includingResourceValuesForKeys: nil,
            relativeTo: nil
        )

        settings.backgroundImagePath = url.path
        settings.backgroundImageBookmark = bookmark
    }

    func addExcludedFolderPath(url: URL) {
        let normalizedPath = normalizeExcludedFolderPath(url.path)
        guard !normalizedPath.isEmpty else {
            return
        }
        if excludedFolderPaths.contains(normalizedPath) {
            return
        }
        excludedFolderPaths.append(normalizedPath)
        excludedFolderPaths.sort { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
    }

    func removeExcludedFolderPath(_ path: String) {
        let normalizedPath = normalizeExcludedFolderPath(path)
        excludedFolderPaths.removeAll { $0 == normalizedPath }
    }

    deinit {
        scopedBackgroundURL?.stopAccessingSecurityScopedResource()
    }

    private func save() {
        guard let data = try? JSONEncoder().encode(settings) else { return }
        UserDefaults.standard.set(data, forKey: defaultsKey)
    }

    private func applyThemeOverridesFromConfigFile() {
        guard let raw = try? String(contentsOf: Self.configPath(), encoding: .utf8) else {
            return
        }

        // Config file parsing with graceful fallback:
        // Invalid values are silently ignored and default values are used instead.
        // This ensures the app remains functional even with corrupted config.

        excludedFolderPaths = []

        for line in raw.split(whereSeparator: \ .isNewline) {
            let stripped = stripComment(String(line)).trimmingCharacters(in: .whitespacesAndNewlines)
            if stripped.isEmpty {
                continue
            }

            guard let splitPoint = stripped.firstIndex(of: "=") else {
                continue
            }

            let key = stripped[..<splitPoint].trimmingCharacters(in: .whitespacesAndNewlines)
            let value = stripped[stripped.index(after: splitPoint)...].trimmingCharacters(in: .whitespacesAndNewlines)

            switch key {
            case "ui_theme":
                settings.themeName = value
            case "ui_tint_red":
                if let parsed = parseUnitDouble(value) {
                    settings.tintRed = parsed
                }
            case "ui_tint_green":
                if let parsed = parseUnitDouble(value) {
                    settings.tintGreen = parsed
                }
            case "ui_tint_blue":
                if let parsed = parseUnitDouble(value) {
                    settings.tintBlue = parsed
                }
            case "ui_tint_opacity":
                if let parsed = parseUnitDouble(value) {
                    settings.tintOpacity = parsed
                }
            case "ui_blur_material":
                if let material = parseBlurMaterial(value) {
                    settings.blurMaterial = material
                }
            case "ui_blur_opacity":
                if let parsed = parseUnitDouble(value) {
                    settings.blurOpacity = parsed
                }
            case "ui_font_name":
                if !value.isEmpty {
                    settings.fontName = value
                }
            case "ui_font_size":
                if let parsed = parsePositiveDouble(value) {
                    settings.fontSize = parsed
                }
            case "ui_font_red":
                if let parsed = parseUnitDouble(value) {
                    settings.fontRed = parsed
                }
            case "ui_font_green":
                if let parsed = parseUnitDouble(value) {
                    settings.fontGreen = parsed
                }
            case "ui_font_blue":
                if let parsed = parseUnitDouble(value) {
                    settings.fontBlue = parsed
                }
            case "ui_font_opacity":
                if let parsed = parseUnitDouble(value) {
                    settings.fontOpacity = parsed
                }
            case "ui_border_thickness":
                if let parsed = parsePositiveDouble(value) {
                    settings.borderThickness = parsed
                }
            case "ui_border_red":
                if let parsed = parseUnitDouble(value) {
                    settings.borderRed = parsed
                }
            case "ui_border_green":
                if let parsed = parseUnitDouble(value) {
                    settings.borderGreen = parsed
                }
            case "ui_border_blue":
                if let parsed = parseUnitDouble(value) {
                    settings.borderBlue = parsed
                }
            case "ui_border_opacity":
                if let parsed = parseUnitDouble(value) {
                    settings.borderOpacity = parsed
                }
            case "file_scan_depth":
                if let parsed = parsePositiveInt(value) {
                    settings.fileScanDepth = parsed
                }
            case "file_scan_limit":
                if let parsed = parsePositiveInt(value) {
                    settings.fileScanLimit = parsed
                }
            case "lazy_indexing_enabled":
                if let parsed = parseBool(value) {
                    settings.lazyIndexingEnabled = parsed
                }
            case "file_exclude_paths":
                excludedFolderPaths = parseExcludedFolderPaths(value)
            case "backend_log_level":
                if let parsed = parseBackendLogLevel(value) {
                    settings.backendLogLevel = parsed
                }
            case "launch_at_login":
                if let parsed = parseBool(value) {
                    settings.launchAtLogin = parsed
                }
            case "ui_background_image":
                if !value.isEmpty {
                    settings.backgroundImagePath = value
                }
            case "ui_background_image_mode":
                if let mode = BackgroundImageMode(rawValue: value.lowercased()) {
                    settings.backgroundImageMode = mode
                }
            case "ui_background_image_opacity":
                if let parsed = parseUnitDouble(value) {
                    settings.backgroundImageOpacity = parsed
                }
            case "ui_background_image_blur":
                if let parsed = parsePositiveDouble(value) {
                    settings.backgroundImageBlur = parsed
                }
            case "settings_blur_multiplier":
                if let parsed = parseUnitDouble(value) {
                    settings.settingsBlurMultiplier = parsed
                }
            default:
                continue
            }
        }
    }

    private static func configPath() -> URL {
        let env = ProcessInfo.processInfo.environment
        if let custom = env["LOOK_CONFIG_PATH"]?.trimmingCharacters(in: .whitespacesAndNewlines),
            !custom.isEmpty
        {
            return URL(fileURLWithPath: custom)
        }

        let home = env["HOME"] ?? NSHomeDirectory()
        return URL(fileURLWithPath: home).appendingPathComponent(".look.config")
    }

    private static func ensureDefaultConfigFileExists(at path: URL) {
        if FileManager.default.fileExists(atPath: path.path) {
            return
        }

        try? defaultConfigContents.write(to: path, atomically: true, encoding: .utf8)
    }

    private func stripComment(_ line: String) -> String {
        guard let index = line.firstIndex(of: "#") else {
            return line
        }
        return String(line[..<index])
    }

    private func parseUnitDouble(_ value: String) -> Double? {
        guard let parsed = Double(value), (0...1).contains(parsed) else {
            return nil
        }
        return parsed
    }

    private func parsePositiveDouble(_ value: String) -> Double? {
        guard let parsed = Double(value), parsed > 0 else {
            return nil
        }
        return parsed
    }

    private func parsePositiveInt(_ value: String) -> Int? {
        guard let parsed = Int(value), parsed > 0 else {
            return nil
        }
        return parsed
    }

    private func parseBool(_ value: String) -> Bool? {
        switch value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "1", "true", "yes", "on":
            return true
        case "0", "false", "no", "off":
            return false
        default:
            return nil
        }
    }

    private func parseBackendLogLevel(_ value: String) -> BackendLogLevel? {
        BackendLogLevel(rawValue: value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    }

    private func parseBlurMaterial(_ value: String) -> LauncherBlurMaterial? {
        switch value.lowercased() {
        case "hudwindow", "high_contrast", "high-contrast":
            return .hudWindow
        case "sidebar", "soft":
            return .sidebar
        case "menu", "balanced":
            return .menu
        case "underwindowbackground", "under_window_background", "subtle":
            return .underWindowBackground
        default:
            return LauncherBlurMaterial(rawValue: value)
        }
    }

    private func upsertConfigLine(_ lines: inout [String], key: String, value: String) {
        let wanted = "\(key)="
        for index in lines.indices {
            let trimmed = stripComment(lines[index]).trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.hasPrefix(wanted) {
                lines[index] = "\(key)=\(value)"
                return
            }
        }
        lines.append("\(key)=\(value)")
    }

    private func removeConfigLine(_ lines: inout [String], key: String) {
        let wanted = "\(key)="
        lines.removeAll { line in
            let trimmed = stripComment(line).trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.hasPrefix(wanted)
        }
    }

    private func parseExcludedFolderPaths(_ value: String) -> [String] {
        var seen = Set<String>()
        var paths: [String] = []
        for token in parseCSVTokens(value) {
            let normalized = normalizeExcludedFolderPath(token)
            if normalized.isEmpty || seen.contains(normalized) {
                continue
            }
            seen.insert(normalized)
            paths.append(normalized)
        }
        return paths.sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
    }

    private func normalizeExcludedFolderPath(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return ""
        }
        let expanded = expandConfigLikePath(trimmed)
        return URL(fileURLWithPath: expanded).standardizedFileURL.path
    }

    private func escapeCSVToken(_ value: String) -> String {
        value
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: ",", with: "\\,")
    }

    private func parseCSVTokens(_ value: String) -> [String] {
        var tokens: [String] = []
        var current = ""
        var escaping = false

        for character in value {
            if escaping {
                current.append(character)
                escaping = false
                continue
            }
            if character == "\\" {
                escaping = true
                continue
            }
            if character == "," {
                let trimmed = current.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    tokens.append(trimmed)
                }
                current = ""
                continue
            }
            current.append(character)
        }

        if escaping {
            current.append("\\")
        }

        let trimmed = current.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            tokens.append(trimmed)
        }

        return tokens
    }

    private func expandConfigLikePath(_ value: String) -> String {
        let home = NSHomeDirectory()
        if value == "~" {
            return home
        }
        if value.hasPrefix("~/") {
            let relative = value.dropFirst(2)
            return home + "/" + relative
        }
        if value.hasPrefix("/") {
            return value
        }
        return home + "/" + value
    }

    private func resolveUsableFontName(_ input: String) -> String? {
        if let exact = NSFont(name: input, size: 12) {
            return exact.fontName
        }

        let manager = NSFontManager.shared
        if let members = manager.availableMembers(ofFontFamily: input),
            let postscriptName = extractPostScriptName(from: members)
        {
            return postscriptName
        }

        let lowercasedInput = input.lowercased()
        for family in NSFontManager.shared.availableFontFamilies {
            if family.lowercased() == lowercasedInput,
                let members = manager.availableMembers(ofFontFamily: family),
                let postscriptName = extractPostScriptName(from: members)
            {
                return postscriptName
            }
        }

        return nil
    }

    private func extractPostScriptName(from members: [[Any]]) -> String? {
        guard let firstMember = members.first,
            let postScript = firstMember.first as? String,
            !postScript.isEmpty
        else {
            return nil
        }
        return postScript
    }

    private static let defaultConfigContents = """
# look configuration
# Generated on first launch. Edit values and press Cmd+Shift+; to reload.

# Backend indexing
app_scan_roots=/Applications,/System/Applications,/System/Applications/Utilities,/System/Library/CoreServices/Applications,/System/Library/CoreServices/Finder.app/Contents/Applications
app_scan_depth=3
app_exclude_paths=
app_exclude_names=
file_scan_roots=Desktop,Documents,Downloads
file_scan_depth=4
file_scan_limit=8000
lazy_indexing_enabled=true
file_exclude_paths=
backend_log_level=error
launch_at_login=true
skip_dir_names=node_modules,target,build,dist,library,applications,old firefox data,deriveddata,pods,vendor,out,coverage,tmp,cache,venv

# UI theme
ui_tint_red=0.08
ui_tint_green=0.10
ui_tint_blue=0.12
ui_tint_opacity=0.55
ui_blur_material=hudWindow
ui_blur_opacity=0.95
ui_font_name=SF Pro Text
ui_font_size=14
ui_font_red=0.96
ui_font_green=0.96
ui_font_blue=0.98
ui_font_opacity=0.96
ui_border_thickness=1.0
ui_border_red=1.0
ui_border_green=1.0
ui_border_blue=1.0
ui_border_opacity=0.12

# Search aliases (apps + System Settings). Format: alias_<keyword>=Term1|Term2|Term3
alias_note=Notion|Obsidian|Notes|Apple Notes|Bear|Logseq
alias_code=Visual Studio Code|VSCode|Cursor|Windsurf|IntelliJ IDEA|PyCharm|WebStorm|Neovim|Xcode|Zed
alias_term=Terminal|iTerm|iTerm2|Ghostty|WezTerm|Alacritty|Kitty|Warp
alias_chat=Slack|Discord|Telegram|Messages
alias_music=Spotify|Apple Music|Music
alias_brow=Safari|Arc|Google Chrome|Chrome|Firefox|Brave
"""

    private static func loadThemeSettings(from data: Data?) -> ThemeSettings {
        guard let data else {
            return .default
        }

        let decoder = JSONDecoder()
        if let decoded = try? decoder.decode(ThemeSettings.self, from: data) {
            return decoded
        }

        guard
            var object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
            object["lazyIndexingEnabled"] == nil
        else {
            return .default
        }

        object["lazyIndexingEnabled"] = true
        guard
            let migratedData = try? JSONSerialization.data(withJSONObject: object),
            let migrated = try? decoder.decode(ThemeSettings.self, from: migratedData)
        else {
            return .default
        }
        return migrated
    }

    private func refreshBackgroundImageURL() {
        scopedBackgroundURL?.stopAccessingSecurityScopedResource()
        scopedBackgroundURL = nil
        backgroundImageURL = nil
        backgroundImage = nil

        if let bookmark = settings.backgroundImageBookmark {
            var isStale = false
            if let resolved = try? URL(
                resolvingBookmarkData: bookmark,
                options: .withSecurityScope,
                relativeTo: nil,
                bookmarkDataIsStale: &isStale
            ) {
                _ = resolved.startAccessingSecurityScopedResource()
                scopedBackgroundURL = resolved
                backgroundImageURL = resolved
                backgroundImage = NSImage(contentsOf: resolved)
                return
            }
        }

        if let path = settings.backgroundImagePath {
            let url = URL(fileURLWithPath: path)
            backgroundImageURL = url
            backgroundImage = NSImage(contentsOf: url)
        }
    }

    private func applyLaunchAtLoginSetting() -> Bool {
        if #available(macOS 13.0, *) {
            do {
                if settings.launchAtLogin {
                    if SMAppService.mainApp.status != .enabled {
                        try SMAppService.mainApp.register()
                    }
                } else if SMAppService.mainApp.status == .enabled {
                    try SMAppService.mainApp.unregister()
                }
                return true
            } catch {
                return false
            }
        }

        return false
    }
}
