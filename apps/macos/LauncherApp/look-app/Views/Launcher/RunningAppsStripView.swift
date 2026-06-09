import AppKit
import SwiftUI

struct RunningAppsStripView: View {
    typealias Layout = AppConstants.Launcher.RunningAppsStrip

    @ObservedObject var service: RunningAppsService
    let themeStore: ThemeStore
    let onActivate: (Int) -> Void

    @State private var hoveredIndex: Int?

    var body: some View {
        // A single horizontal row living in the right half of the search bar.
        // Right-aligned: icons hug the right edge and grow leftward as more apps
        // appear. The leading Spacer pushes them right; itemGap keeps the
        // inter-icon spacing.
        HStack(spacing: Layout.itemGap) {
            Spacer(minLength: 0)
            iconStack
        }
        .padding(.horizontal, Layout.verticalPadding)
        .padding(.vertical, Layout.horizontalPadding)
        .frame(height: Layout.width)
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder
    private var iconStack: some View {
        let total = service.items.count
        ForEach(0..<total, id: \.self) { index in
            let item = service.items[index]
            let shortcutNumber = Layout.ergonomicKey(forVisualPosition: index, total: total)
            RunningAppIconItem(
                item: item,
                shortcutNumber: shortcutNumber,
                isActive: service.activePID == item.id,
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

private struct RunningAppIconItem: View {
    let item: RunningAppItem
    let shortcutNumber: Int
    let isActive: Bool
    let isHovered: Bool
    let themeStore: ThemeStore
    let onTap: () -> Void
    let onHoverChange: (Bool) -> Void

    private let iconSize: CGFloat = AppConstants.Launcher.RunningAppsStrip.iconSize
    private var iconCornerRadius: CGFloat { iconSize * 0.22 }

    var body: some View {
        ZStack {
            iconView
                .overlay(alignment: .topTrailing) { badge }
                .overlay { activeRing }
                .scaleEffect(isHovered ? 1.12 : 1.0)
                .opacity(isActive ? 1.0 : (isHovered ? 0.95 : 0.75))
                .animation(.easeOut(duration: 0.15), value: isHovered)
                .animation(.easeOut(duration: 0.15), value: isActive)
                .contentShape(Rectangle())
                .onTapGesture { onTap() }
                .onHover { hovering in onHoverChange(hovering) }
                .help(tooltipText)
        }
        .frame(width: iconSize, height: iconSize)
    }

    @ViewBuilder
    private var iconView: some View {
        if let icon = item.icon {
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
                    Text(String(item.name.prefix(1)).uppercased())
                        .font(themeStore.uiFont(size: 12, weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                }
        }
    }

    @ViewBuilder
    private var badge: some View {
        Text("\(shortcutNumber)")
            .font(.system(size: 9, weight: .bold, design: .monospaced))
            .foregroundStyle(themeStore.fontColor())
            .frame(width: 14, height: 14)
            .background(
                RoundedRectangle(cornerRadius: 7)
                    .fill(Color.black.opacity(0.72))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 7)
                    .strokeBorder(themeStore.fontColor(opacityMultiplier: 0.45), lineWidth: 1)
            )
            .offset(x: 5, y: -3)
    }

    @ViewBuilder
    private var activeRing: some View {
        if isActive {
            RoundedRectangle(cornerRadius: iconCornerRadius + 3, style: .continuous)
                .strokeBorder(themeStore.accentColor().opacity(0.5), lineWidth: 1.5)
                .padding(-3)
        }
    }

    private var tooltipText: String {
        "\(item.name) ⌘\(shortcutNumber)"
    }
}
