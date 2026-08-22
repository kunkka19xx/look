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

    /// Hidden under the selection pill and after the final row. The row keeps
    /// the divider's height either way, so selection never reflows the list.
    private var showsDivider: Bool {
        !isLast && !isSelected
    }

    private var syntheticRow: SyntheticRow? {
        SyntheticRow.classify(resultID: result.id)
    }

    /// A row a user-declared block produced (`specs/user-sources.md`).
    private var isSourceRow: Bool {
        result.id.hasPrefix(AppConstants.Launcher.SourceBlock.idPrefix)
    }

    /// Cached: this runs inside `body`, and a fresh `NSImage` per redraw makes
    /// every icon in the list flicker. See `RowIconCache`.
    private var rowIcon: NSImage {
        switch syntheticRow {
        case .commandSuggestion:
            return RowIconCache.image(key: "symbol:terminal") {
                NSImage(systemSymbolName: "terminal", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        case .calc:
            return RowIconCache.image(key: "feature:calc") { LauncherCalcFeature.icon() }
        case .webURL:
            return RowIconCache.image(key: "symbol:globe") {
                NSImage(systemSymbolName: "globe", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        case .meeting:
            return RowIconCache.image(key: "symbol:video") {
                NSImage(systemSymbolName: "video.fill", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        case .call(let url):
            let symbol = LinkRowAppearance.symbol(forURL: url)
            return RowIconCache.image(key: "symbol:\(symbol)") {
                NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        case .prefixSuggestion, .webSuggestion:
            return RowIconCache.image(key: "symbol:magnifyingglass") {
                NSImage(systemSymbolName: "magnifyingglass", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        case .aiAction(let toolID):
            // Keyed per tool: each one has its own symbol.
            return RowIconCache.image(key: "aiaction:\(toolID)") {
                AIActionAppearance.icon(forToolID: toolID)
            }
        case nil:
            break
        }

        if result.kind == .clipboard {
            return RowIconCache.image(key: "symbol:doc.on.clipboard") {
                NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: nil)
                    ?? NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        }

        // A declared block has no file to take an icon from, and must not look
        // like one: Enter performs steps rather than opening anything. What the
        // block declared wins; the bolt is the fallback.
        if result.kind == .action {
            if let declared = SourceBlockIcons.declaredIcon(
                SourceBlockCatalog.icon(forCandidateID: result.id)
            ) {
                return declared
            }
            return RowIconCache.image(key: "symbol:bolt") {
                NSImage(systemSymbolName: "bolt.fill", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
            }
        }

        // Not cached: a process icon is keyed to a live pid, and pids are reused.
        if result.kind == .process, let pid = result.processPID {
            return LauncherProcessFeature.icon(forPID: pid)
        }

        if result.id.hasPrefix("setting:") {
            let settingsPath = "/System/Applications/System Settings.app"
            if FileManager.default.fileExists(atPath: settingsPath) {
                return RowIconCache.icon(forFile: settingsPath)
            }
            let legacyPath = "/System/Applications/System Preferences.app"
            return RowIconCache.icon(forFile: legacyPath)
        }
        return RowIconCache.icon(forFile: result.path)
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
        case .action:
            return "Action"
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
        if result.kind == .action {
            // "Action • 3 steps": there is no path, and the step count says this
            // will do several things before the user commits to it.
            return [kindLabel, result.subtitle]
                .compactMap { $0 }
                .joined(separator: "  •  ")
        }
        // A row a user's block produced says WHICH block, not "Folder". The kind
        // is already on the icon, and where the row came from is the thing the
        // list cannot otherwise tell you.
        if isSourceRow, let block = result.subtitle {
            return "\(block)  •  \(pathInfo)"
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
                    RowIcon(image: rowIcon, isSelected: isSelected)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(result.title)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                            .foregroundStyle(themeStore.fontColor())
                        Text(metaLabel)
                            .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                    }
                    // Keyed on `isSelected` so it rides nav's glide transaction.
                    // Offset, not padding: the text must not reflow.
                    .offset(x: isSelected ? Motion.Selection.titleShift : 0)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
            }
            .buttonStyle(.plain)
            .focusable(false)
            // Glides when the change is wrapped in `Motion.Selection.glide`
            // (keyboard nav), snaps otherwise (click, refresh).
            .selectionPill(
                isSelected: isSelected,
                themeStore: themeStore,
                namespace: selectionNamespace)

            Rectangle()
                .fill(themeStore.dividerColor().opacity(Layout.dividerOpacity))
                .frame(height: Layout.dividerHeight)
                .padding(.horizontal, Layout.dividerInset)
                .opacity(showsDivider ? 1 : 0)
        }
    }
}

/// The row's icon, popping with the selection.
///
/// Its own view because a view cannot read an environment value its own body
/// sets, and `selectionPill` publishes the zoom from inside `LauncherRowView`.
private struct RowIcon: View {
    @Environment(\.isSelectionZoomed) private var zoomed
    let image: NSImage
    let isSelected: Bool

    var body: some View {
        Image(nsImage: image)
            .resizable()
            .frame(width: 22, height: 22)
            .scaleEffect(isSelected && zoomed ? Motion.Selection.iconZoomScale : 1)
    }
}
