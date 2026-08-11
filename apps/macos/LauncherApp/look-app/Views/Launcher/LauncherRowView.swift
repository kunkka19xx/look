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

    /// Drives the one-shot zoom the pill and icon do as this row takes the
    /// selection. Local to the row, so a row that is not gaining selection has
    /// nothing to animate.
    @State private var zoomed = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private func zoom() {
        guard !reduceMotion else { return }
        withAnimation(Motion.Selection.zoomIn) {
            zoomed = true
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Motion.Selection.zoomInSeconds) {
            withAnimation(Motion.Selection.zoomOut) {
                zoomed = false
            }
        }
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

    /// Resolved through `RowIconCache`: this runs inside `body`, so every redraw
    /// of the list would otherwise mint a fresh `NSImage` for every visible row
    /// and make all of them flicker. See the note on the cache.
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
                    // Driven by `isSelected` rather than the one-shot state, so it
                    // rides the glide transaction nav already wraps the selection
                    // in and slides rather than snapping. Offset only, so the text
                    // never reflows and neighbouring rows stay put.
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
            // Only the row that just became selected runs this, so exactly one
            // row moves per keypress. No `.animation(_:value:)` anywhere in the
            // row: applied per-row it fires on every neighbour as the selection
            // passes, which is what made the whole list look like it flickered.
            .onChange(of: isSelected) { _, selected in
                // Rows are recycled by the LazyVStack, so a view can arrive
                // carrying a stale `zoomed` from whichever row it was before.
                // Clearing on deselect, and gating the scale on `isSelected`
                // above, means an unselected row can never show a transform.
                guard selected else {
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
