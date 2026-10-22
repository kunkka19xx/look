import SwiftUI

/// A reusable hover tooltip. Attach to any view with `.hoverTooltip("…")` and a
/// bubble appears while the pointer is over it.
///
/// It uses a `popover` rather than an `.overlay` so the bubble renders in its own
/// window and is always drawn on top - an overlay can be covered by sibling
/// views that come later in a VStack/ZStack (e.g. a picker below it).
struct HoverTooltipModifier: ViewModifier {
    let text: String
    var width: CGFloat
    var edge: Edge

    @EnvironmentObject private var themeStore: ThemeStore
    @State private var isHovering = false

    func body(content: Content) -> some View {
        content
            .onHover { hovering in
                isHovering = hovering
            }
            .popover(isPresented: $isHovering, arrowEdge: edge) {
                Text(text)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(.primary)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(width: width, alignment: .leading)
                    .padding(12)
            }
    }
}

extension View {
    /// Shows a floating tooltip bubble with `text` while this view is hovered.
    /// - Parameters:
    ///   - text: The tooltip body.
    ///   - width: Bubble width; the text wraps to fit. Defaults to 280.
    ///   - edge: Which edge the bubble points from. Defaults to `.trailing`.
    func hoverTooltip(_ text: String, width: CGFloat = 280, edge: Edge = .trailing) -> some View {
        modifier(HoverTooltipModifier(text: text, width: width, edge: edge))
    }
}
