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
