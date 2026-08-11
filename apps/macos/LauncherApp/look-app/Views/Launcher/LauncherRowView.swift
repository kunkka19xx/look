import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct LauncherRowView: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let result: LauncherResult
    let isSelected: Bool
    let isPicked: Bool
    /// Last row in the list, so the trailing divider is suppressed.
    let isLast: Bool
    /// Shared namespace for the single selection pill that glides between rows
    /// (see `Motion.Selection`). Only the selected row is the geometry source.
    let selectionNamespace: Namespace.ID
    let onOpen: () -> Void

    private enum Layout {
        static let cornerRadius: CGFloat = 8
        static let borderWidth: CGFloat = 1
        static let dividerHeight: CGFloat = 1
        static let dividerInset: CGFloat = 6
        static let dividerOpacity: Double = 0.8
    }

    private var pillCornerRadius: CGFloat {
        themeStore.surfaceCornerRadius(Layout.cornerRadius)
    }

    /// The trailing hairline is suppressed under the selection pill (a rule
    /// running through a filled pill reads as an artifact) and after the final
    /// row (nothing follows it to separate). The row keeps the divider's height
    /// either way and only fades it, so selection never reflows the list.
    private var showsDivider: Bool {
        !isLast && !isSelected
    }

    private var syntheticRow: SyntheticRow? {
        SyntheticRow.classify(resultID: result.id)
    }

    private var rowIcon: NSImage {
        switch syntheticRow {
        case .commandSuggestion:
            return NSImage(systemSymbolName: "terminal", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        case .calc:
            return LauncherCalcFeature.icon()
        case .webURL:
            return NSImage(systemSymbolName: "globe", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        case .prefixSuggestion, .webSuggestion:
            return NSImage(systemSymbolName: "magnifyingglass", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        case nil:
            break
        }

        if result.kind == .clipboard {
            return NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: nil)
                ?? NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
        }

        if result.kind == .process, let pid = result.processPID {
            return LauncherProcessFeature.icon(forPID: pid)
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
        case .process:
            return "Process"
        }
    }

    private var metaLabel: String {
        if syntheticRow != nil {
            return result.subtitle ?? ""
        }
        if result.kind == .clipboard {
            return result.subtitle ?? kindLabel
        }
        if result.kind == .process {
            // "PID 1234 · :3000" - carries the pid and any listening ports.
            return result.subtitle ?? kindLabel
        }
        if result.kind == .app {
            return kindLabel
        }
        return "\(kindLabel)  •  \(pathInfo)"
    }

    var body: some View {
        VStack(spacing: 0) {
            Button(action: onOpen) {
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
                .contentShape(Rectangle())
            }
            .buttonStyle(PressableSurfaceStyle())
            // The row used to be a bare tap gesture, which never took focus.
            // A Button would accept it under Full Keyboard Access and pull the
            // caret out of the search field, so keep the old focus behaviour.
            .focusable(false)
            // The pill stays outside the button so the press scale can't perturb
            // the frame `matchedGeometryEffect` reports for the glide.
            .background {
                // One pill shared across rows via matchedGeometryEffect. It
                // glides when the selection change is wrapped in
                // `Motion.Selection.glide` (keyboard nav) and snaps otherwise
                // (click, results refresh).
                if isSelected {
                    RoundedRectangle(cornerRadius: pillCornerRadius, style: .continuous)
                        .fill(themeStore.selectionFillColor())
                        .overlay {
                            RoundedRectangle(cornerRadius: pillCornerRadius, style: .continuous)
                                .stroke(themeStore.dividerColor(), lineWidth: Layout.borderWidth)
                        }
                        .matchedGeometryEffect(id: Motion.Selection.geometryID, in: selectionNamespace)
                }
            }

            Rectangle()
                .fill(themeStore.dividerColor().opacity(Layout.dividerOpacity))
                .frame(height: Layout.dividerHeight)
                .padding(.horizontal, Layout.dividerInset)
                .opacity(showsDivider ? 1 : 0)
                .animation(Motion.Value.dividerFade, value: showsDivider)
        }
    }
}
