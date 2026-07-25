import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct LauncherRowView: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let result: LauncherResult
    let isSelected: Bool
    let isPicked: Bool
    /// Shared namespace for the single selection pill that glides between rows
    /// (see `Motion.Selection`). Only the selected row is the geometry source.
    let selectionNamespace: Namespace.ID
    let onOpen: () -> Void

    private enum Layout {
        static let cornerRadius: CGFloat = 8
        static let borderWidth: CGFloat = 1
    }

    private var isPrefixSuggestion: Bool {
        result.id.hasPrefix(AppConstants.Launcher.PrefixSuggestion.resultIDPrefix)
    }

    private var isWebSuggestion: Bool {
        result.id.hasPrefix(AppConstants.Launcher.WebSuggestion.resultIDPrefix)
    }

    private var isCommandSuggestion: Bool {
        result.id.hasPrefix(AppConstants.Launcher.CommandSuggestion.resultIDPrefix)
    }

    private var isURLResult: Bool {
        result.id.hasPrefix(AppConstants.Launcher.WebURL.resultIDPrefix)
    }

    private var rowIcon: NSImage {
        if isCommandSuggestion {
            return NSImage(systemSymbolName: "terminal", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        }

        if isURLResult {
            return NSImage(systemSymbolName: "globe", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        }

        if isPrefixSuggestion || isWebSuggestion {
            return NSImage(systemSymbolName: "magnifyingglass", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        }

        if result.kind == .clipboard {
            return NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: nil)
                ?? NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        }

        if result.id.hasPrefix("setting:") {
            let settingsPath = "/System/Applications/System Settings.app"
            if FileManager.default.fileExists(atPath: settingsPath) {
                return NSWorkspace.shared.icon(forFile: settingsPath)
            }
            let legacyPath = "/System/Applications/System Preferences.app"
            return NSWorkspace.shared.icon(forFile: legacyPath)
        }
        return NSWorkspace.shared.icon(forFile: result.path)
    }

    private var pathInfo: String {
        let parentPath = URL(fileURLWithPath: result.path).deletingLastPathComponent().path
        let components = parentPath
            .split(separator: "/")
            .map(String.init)
        let tail = components.suffix(3).joined(separator: "/")

        if tail.isEmpty {
            return "/"
        }
        if components.count > 3 {
            return ".../\(tail)"
        }
        return "/\(tail)"
    }

    private var kindLabel: String {
        switch result.kind {
        case .app:
            return "App"
        case .file:
            return "File"
        case .folder:
            return "Folder"
        case .clipboard:
            return "Clipboard"
        }
    }

    private var metaLabel: String {
        if isPrefixSuggestion || isWebSuggestion || isCommandSuggestion || isURLResult {
            return result.subtitle ?? ""
        }
        if result.kind == .clipboard {
            return result.subtitle ?? kindLabel
        }
        if result.kind == .app {
            return kindLabel
        }
        return "\(kindLabel)  •  \(pathInfo)"
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                if isPicked {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(themeStore.selectionFillColor())
                        .frame(width: 14)
                }
                Image(nsImage: rowIcon)
                    .resizable()
                    .frame(width: 22, height: 22)
                VStack(alignment: .leading, spacing: 2) {
                    Text(result.title)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                        .foregroundStyle(themeStore.fontColor())
                    Text(metaLabel)
                        .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background {
                // One pill shared across rows via matchedGeometryEffect. It
                // glides when the selection change is wrapped in
                // `Motion.Selection.glide` (keyboard nav) and snaps otherwise
                // (click, results refresh).
                if isSelected {
                    RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
                        .fill(themeStore.selectionFillColor())
                        .overlay {
                            RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
                                .stroke(themeStore.dividerColor(), lineWidth: Layout.borderWidth)
                        }
                        .matchedGeometryEffect(id: Motion.Selection.geometryID, in: selectionNamespace)
                }
            }
            .contentShape(Rectangle())
            .onTapGesture {
                onOpen()
            }

            Rectangle()
                .fill(themeStore.dividerColor().opacity(0.8))
                .frame(height: 1)
                .padding(.horizontal, 6)
        }
    }
}
