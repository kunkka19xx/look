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

    /// Drives the one-shot zoom as this row takes the selection.
    @State private var zoomed = false
    /// Bumped on every zoom and on deselect, so a pending reset that belongs to
    /// an earlier zoom cannot cut short a newer one. Reachable by arrowing away
    /// and back inside `zoomInSeconds`.
    @State private var zoomGeneration = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private func zoom() {
        guard !reduceMotion else { return }
        zoomGeneration &+= 1
        let generation = zoomGeneration
        withAnimation(Motion.Selection.zoomIn) {
            zoomed = true
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Motion.Selection.zoomInSeconds) {
            guard zoomGeneration == generation else { return }
            withAnimation(Motion.Selection.zoomOut) {
                zoomed = false
            }
        }
    }

    /// Hidden under the selection pill and after the final row. The row keeps
    /// the divider's height either way, so selection never reflows the list.
    private var showsDivider: Bool {
        !isLast && !isSelected
    }

    private var syntheticRow: SyntheticRow? {
        SyntheticRow.classify(resultID: result.id)
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
        case .prefixSuggestion, .webSuggestion:
            return RowIconCache.image(key: "symbol:magnifyingglass") {
                NSImage(systemSymbolName: "magnifyingglass", accessibilityDescription: nil)
                    ?? NSWorkspace.shared.icon(for: .plainText)
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
                        .scaleEffect(isSelected && zoomed ? Motion.Selection.iconZoomScale : 1)
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
                        .scaleEffect(isSelected && zoomed ? Motion.Selection.pillZoomScale : 1)
                }
            }
            // Deliberately no `.animation(_:value:)` in this row: per-row it
            // fires on every neighbour as the selection passes, flickering the
            // whole list. Clearing on deselect covers LazyVStack recycling,
            // where a view can arrive holding a previous row's `zoomed`.
            .onChange(of: isSelected) { _, selected in
                guard selected else {
                    zoomGeneration &+= 1
                    zoomed = false
                    return
                }
                zoom()
            }

            Rectangle()
                .fill(themeStore.dividerColor().opacity(Layout.dividerOpacity))
                .frame(height: Layout.dividerHeight)
                .padding(.horizontal, Layout.dividerInset)
                .opacity(showsDivider ? 1 : 0)
        }
    }
}
