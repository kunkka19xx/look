import AppKit
import SwiftUI

struct ClipboardHistoryView: View {
    let entries: [ClipboardEntry]
    let selectedID: String?
    let themeStore: ThemeStore
    let onSelect: (String) -> Void
    let onPaste: (String) -> Void
    let onDelete: (String) -> Void
    let onTogglePin: (String) -> Void

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 4) {
                    ForEach(entries) { entry in
                        ClipboardRowView(
                            entry: entry,
                            isSelected: selectedID == entry.id,
                            onPaste: {
                                onSelect(entry.id)
                                onPaste(entry.id)
                            },
                            onDelete: { onDelete(entry.id) },
                            onTogglePin: { onTogglePin(entry.id) }
                        )
                        .id(entry.id)
                    }
                }
                .padding(2)
            }
            .onChange(of: selectedID) { _, newID in
                guard let newID else { return }
                withAnimation(.easeOut(duration: 0.12)) {
                    proxy.scrollTo(newID, anchor: .center)
                }
            }
        }
    }
}

struct ClipboardRowView: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let entry: ClipboardEntry
    let isSelected: Bool
    let onPaste: () -> Void
    let onDelete: () -> Void
    let onTogglePin: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: entry.contentType.icon)
                    .frame(width: 22, height: 22)
                    .foregroundStyle(iconColor)

                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.displayText)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                        .foregroundStyle(themeStore.fontColor())
                        .lineLimit(2)

                    HStack(spacing: 6) {
                        Text(entry.contentType.label)
                        if let app = entry.sourceApp, !app.isEmpty {
                            Text("•")
                            Text(app)
                        }
                        Text("•")
                        Text(entry.relativeTime)
                        if entry.pinned {
                            Image(systemName: "pin.fill")
                                .font(.system(size: 8))
                        }
                    }
                    .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                    .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.65))
                    .lineLimit(1)
                }

                Spacer(minLength: 0)

                if isSelected {
                    HStack(spacing: 4) {
                        Button(action: onTogglePin) {
                            Image(systemName: entry.pinned ? "pin.slash" : "pin")
                                .font(.system(size: 11))
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.6))

                        Button(action: onDelete) {
                            Image(systemName: "trash")
                                .font(.system(size: 11))
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.red.opacity(0.7))
                    }
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(
                isSelected ? .white.opacity(0.12) : .clear,
                in: RoundedRectangle(cornerRadius: 8, style: .continuous)
            )
            .overlay {
                if isSelected {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(.white.opacity(0.18), lineWidth: 1)
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { onPaste() }

            Rectangle()
                .fill(.white.opacity(0.06))
                .frame(height: 1)
                .padding(.horizontal, 6)
        }
    }

    private var iconColor: Color {
        switch entry.contentType {
        case .text: return .blue
        case .image: return .purple
        case .fileList: return .orange
        }
    }
}
