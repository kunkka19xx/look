import SwiftUI

/// One stored conversation in the AI mode list. Extracted from `LauncherView`
/// so it can carry the same per-row selection state the results rows do (the
/// one-shot zoom needs `@State`, which an inline `ForEach` body cannot hold),
/// and so the giant launcher body stays inside the type checker's budget.
struct ConversationRowView: View {
    let conversation: AIConversation
    let snippet: String
    /// The ⌘-chip shown on the left ("⌘1"), empty past the mapped digits.
    let jumpKey: String
    let isSelected: Bool
    let themeStore: ThemeStore
    let namespace: Namespace.ID
    let onOpen: () -> Void
    let onDelete: () -> Void

    /// Drives the one-shot zoom as this row takes the selection.
    @State private var zoomed = false
    /// Bumped on every zoom and on deselect, so a pending reset belonging to an
    /// earlier zoom cannot cut short a newer one (arrow away and back fast).
    @State private var zoomGeneration = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var fontSize: Double { themeStore.settings.fontSize }

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Text(jumpKey)
                .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .semibold))
                .foregroundStyle(themeStore.accentColor())
                .frame(minWidth: 22, alignment: .leading)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    Text(conversation.displayTitle())
                        .font(themeStore.uiFont(size: CGFloat(fontSize - 1), weight: .medium))
                        .foregroundStyle(themeStore.fontColor())
                        .lineLimit(1)
                    Spacer()
                    Text(conversation.updatedAt.formatted(.relative(presentation: .named)))
                        .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
                if !snippet.isEmpty {
                    Text(snippet)
                        .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor().opacity(0.75))
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }
            }
            // The selected row's text shifts, same as a selected result.
            .offset(x: isSelected ? Motion.Selection.titleShift : 0)

            Button(action: onDelete) {
                Image(systemName: "trash")
                    .font(themeStore.uiFont(size: CGFloat(fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor().opacity(isSelected ? 0.9 : 0.35))
            }
            .buttonStyle(.plain)
            .help("Delete conversation (⌘D or ⌘⌫)")
        }
        .padding(.horizontal, 10)
        // Matches the results rows, so the pill is the same height in both lists.
        .padding(.vertical, 8)
        .background {
            RoundedRectangle(
                cornerRadius: themeStore.surfaceCornerRadius(SelectionPill.Metrics.cornerRadius),
                style: .continuous
            )
            .fill(themeStore.surfaceFill(0.55))
            if isSelected {
                SelectionPill(
                    themeStore: themeStore,
                    namespace: namespace,
                    geometryID: Self.geometryID,
                    zoomed: zoomed)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture(perform: onOpen)
        // No `.animation(_:value:)` here: per-row it fires on every neighbour as
        // the selection passes, flickering the whole list.
        .onChange(of: isSelected) { _, selected in
            guard selected else {
                zoomGeneration &+= 1
                zoomed = false
                return
            }
            guard !reduceMotion else { return }
            zoomGeneration &+= 1
            let generation = zoomGeneration
            withAnimation(Motion.Selection.zoomIn) { zoomed = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + Motion.Selection.zoomInSeconds) {
                guard generation == zoomGeneration else { return }
                withAnimation(Motion.Selection.zoomOut) { zoomed = false }
            }
        }
    }

    /// Its own pill id: the results list has its own, and one pill must never
    /// try to fly between two lists.
    static let geometryID = "look.session.pill"
}
