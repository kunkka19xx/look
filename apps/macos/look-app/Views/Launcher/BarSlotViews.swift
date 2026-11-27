import AppKit
import SwiftUI

struct AppStripTile: Identifiable {
    let id: String
    let name: String
    let icon: NSImage?
    var isActive = false
}

/// Hugs the right edge of the bar's right half, growing leftward.
private struct BarSlotFrame<Content: View>: View {
    typealias Layout = AppConstants.Launcher.RunningAppsStrip

    let spacing: CGFloat
    @ViewBuilder let content: Content

    var body: some View {
        HStack(spacing: spacing) {
            Spacer(minLength: 0)
            content
        }
        .padding(.horizontal, Layout.verticalPadding)
        .frame(height: Layout.width)
        .frame(maxWidth: .infinity)
    }
}

struct AppStripView: View {
    typealias Layout = AppConstants.Launcher.RunningAppsStrip

    let tiles: [AppStripTile]
    let themeStore: ThemeStore
    let onActivate: (Int) -> Void

    @State private var hoveredIndex: Int?

    var body: some View {
        BarSlotFrame(spacing: Layout.itemGap) {
            let total = tiles.count
            ForEach(Array(tiles.enumerated()), id: \.element.id) { index, tile in
                let shortcutNumber = Layout.ergonomicKey(forVisualPosition: index, total: total)
                AppStripIconItem(
                    tile: tile,
                    shortcutNumber: shortcutNumber,
                    isHovered: hoveredIndex == index,
                    themeStore: themeStore,
                    onTap: { onActivate(shortcutNumber) },
                    onHoverChange: { hovering in
                        hoveredIndex = hovering ? index : (hoveredIndex == index ? nil : hoveredIndex)
                    }
                )
            }
        }
    }
}

struct BarTextSlotView: View {
    typealias Layout = AppConstants.Launcher.RunningAppsStrip

    let lead: String?
    let note: String?
    let themeStore: ThemeStore
    var onTap: (() -> Void)?

    @State private var isHovered = false

    var body: some View {
        BarSlotFrame(spacing: 0) {
            HStack(spacing: Layout.textGap) {
                if let lead {
                    Text(lead)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .foregroundStyle(themeStore.secondaryTextColor())
                }
                if let note {
                    Text(note)
                        .font(.system(size: fontSize, design: .monospaced))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .fixedSize()
                }
            }
            .font(themeStore.uiFont(size: fontSize))
            .opacity(isHovered && onTap != nil ? 1 : Layout.textRestingOpacity)
            .animation(.easeOut(duration: Layout.hoverDuration), value: isHovered)
            .contentShape(Rectangle())
            .onTapGesture { onTap?() }
            .onHover { isHovered = $0 }
            .help([lead, note].compactMap { $0 }.joined(separator: " "))
        }
    }

    private var fontSize: CGFloat {
        CGFloat(themeStore.settings.fontSize - 1)
    }
}

private struct AppStripIconItem: View {
    let tile: AppStripTile
    let shortcutNumber: Int
    let isHovered: Bool
    let themeStore: ThemeStore
    let onTap: () -> Void
    let onHoverChange: (Bool) -> Void

    private let iconSize: CGFloat = AppConstants.Launcher.RunningAppsStrip.iconSize
    private var iconCornerRadius: CGFloat { iconSize * 0.22 }

    var body: some View {
        ZStack {
            iconView
                .keyDigitBadge(shortcutNumber, themeStore: themeStore)
                .overlay { activeRing }
                .scaleEffect(isHovered ? 1.12 : 1.0)
                .opacity(tile.isActive ? 1.0 : (isHovered ? 0.95 : 0.75))
                .animation(.easeOut(duration: 0.15), value: isHovered)
                .animation(.easeOut(duration: 0.15), value: tile.isActive)
                .contentShape(Rectangle())
                .onTapGesture { onTap() }
                .onHover { hovering in onHoverChange(hovering) }
                .help(tooltipText)
        }
        .frame(width: iconSize, height: iconSize)
    }

    @ViewBuilder
    private var iconView: some View {
        if let icon = tile.icon {
            Image(nsImage: icon)
                .resizable()
                .interpolation(.high)
                .frame(width: iconSize, height: iconSize)
                .clipShape(RoundedRectangle(cornerRadius: iconCornerRadius, style: .continuous))
        } else {
            RoundedRectangle(cornerRadius: iconCornerRadius, style: .continuous)
                .fill(themeStore.fontColor(opacityMultiplier: 0.14))
                .frame(width: iconSize, height: iconSize)
                .overlay {
                    Text(String(tile.name.prefix(1)).uppercased())
                        .font(themeStore.uiFont(size: 12, weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                }
        }
    }

    @ViewBuilder
    private var activeRing: some View {
        if tile.isActive {
            RoundedRectangle(cornerRadius: iconCornerRadius + 3, style: .continuous)
                .strokeBorder(themeStore.accentColor().opacity(0.5), lineWidth: 1.5)
                .padding(-3)
        }
    }

    private var tooltipText: String {
        "\(tile.name) ⌘\(shortcutNumber)"
    }
}
