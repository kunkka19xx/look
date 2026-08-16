import SwiftUI

/// THE selection pill, shared by every keyboard-navigable list (results,
/// conversations) so a selected row looks the same wherever it appears. It had
/// drifted: the conversation list drew its own accent wash with no border and a
/// hardcoded corner, which read as a different control from the results list.
///
/// One pill per list, moved between rows by `matchedGeometryEffect`: it GLIDES
/// when the selection change is wrapped in `Motion.Selection.glide` (keyboard
/// nav) and snaps otherwise (click, list refresh).
struct SelectionPill: View {
    let themeStore: ThemeStore
    let namespace: Namespace.ID
    /// One id per list. Two lists on screen must not share one, or the pill
    /// tries to fly between them.
    var geometryID: String = Motion.Selection.geometryID
    /// The one-shot zoom as a row takes the selection.
    var zoomed: Bool = false

    enum Metrics {
        static let cornerRadius: CGFloat = 8
        static let borderWidth: CGFloat = 1
    }

    private var cornerRadius: CGFloat {
        themeStore.surfaceCornerRadius(Metrics.cornerRadius)
    }

    var body: some View {
        RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
            .fill(themeStore.selectionFillColor())
            .overlay {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(themeStore.dividerColor(), lineWidth: Metrics.borderWidth)
            }
            .matchedGeometryEffect(id: geometryID, in: namespace)
            .scaleEffect(zoomed ? Motion.Selection.pillZoomScale : 1)
    }
}

/// THE way a row shows selection: the shared pill, plus the one-shot zoom as
/// the row takes it. Every keyboard-navigable list applies this, so the motion
/// is the same wherever a selection moves.
///
/// A `ViewModifier` and not a helper function because the zoom needs `@State`,
/// which an inline `ForEach` body cannot hold. That is why the conversation row
/// had to become its own type, and why the lists that skipped that extraction
/// (mentions, the join picker) ended up with a pill that glided but never
/// zoomed. As a modifier, any row gets both.
private struct SelectionPillModifier: ViewModifier {
    let isSelected: Bool
    let themeStore: ThemeStore
    let namespace: Namespace.ID
    let geometryID: String

    @State private var zoomed = false
    /// Bumped on every zoom and on deselect, so a pending reset belonging to an
    /// earlier zoom cannot cut short a newer one (arrow away and back fast).
    @State private var generation = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            // Published downward so a row can join in - the results row pops
            // its icon on the same beat - without keeping a second copy of the
            // zoom state that could drift out of step with the pill.
            .environment(\.isSelectionZoomed, zoomed)
            .background {
                if isSelected {
                    SelectionPill(
                        themeStore: themeStore,
                        namespace: namespace,
                        geometryID: geometryID,
                        zoomed: zoomed)
                }
            }
            // No `.animation(_:value:)` here: per-row it fires on every
            // neighbour as the selection passes, flickering the whole list.
            .onChange(of: isSelected) { _, selected in
                guard selected else {
                    generation &+= 1
                    zoomed = false
                    return
                }
                guard !reduceMotion else { return }
                generation &+= 1
                let mine = generation
                withAnimation(Motion.Selection.zoomIn) { zoomed = true }
                DispatchQueue.main.asyncAfter(deadline: .now() + Motion.Selection.zoomInSeconds) {
                    guard mine == generation else { return }
                    withAnimation(Motion.Selection.zoomOut) { zoomed = false }
                }
            }
    }
}

private struct SelectionZoomedKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// True during the one-shot zoom of the row that just took the selection.
    /// Set by `selectionPill`; read by anything inside the row that wants to
    /// move with it.
    var isSelectionZoomed: Bool {
        get { self[SelectionZoomedKey.self] }
        set { self[SelectionZoomedKey.self] = newValue }
    }
}

extension View {
    /// Marks this row as the selected one. `geometryID` is per list: two lists
    /// on screen sharing one would make the pill fly between them.
    func selectionPill(
        isSelected: Bool,
        themeStore: ThemeStore,
        namespace: Namespace.ID,
        geometryID: String = Motion.Selection.geometryID
    ) -> some View {
        modifier(
            SelectionPillModifier(
                isSelected: isSelected,
                themeStore: themeStore,
                namespace: namespace,
                geometryID: geometryID))
    }
}
