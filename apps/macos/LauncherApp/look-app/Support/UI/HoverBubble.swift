import SwiftUI

/// An in-window hover bubble. Attach with `.hoverBubble { … }` and a
/// themed bubble appears above the view while the pointer is over it.
///
/// Complements `hoverTooltip`: that one presents an NSPopover, which is
/// its own window and swallows the first click, so it suits read-only
/// info icons. This bubble renders inside the host window and never
/// hit-tests, so clicks always pass through to the anchor - use it on
/// interactive controls (buttons) that need hover detail.
///
/// Placement: a zero-size anchor pinned at the view's top-leading corner
/// with the bubble growing upward out of it, so the bubble's own height
/// can never push it below the anchor or off the window's bottom edge.
struct HoverBubbleModifier<BubbleContent: View>: ViewModifier {
    let isEnabled: Bool
    let width: CGFloat?
    @ViewBuilder let bubble: () -> BubbleContent

    @EnvironmentObject private var themeStore: ThemeStore
    @State private var hovering = false

    func body(content: Content) -> some View {
        content
            .onHover { hovering = $0 }
            .overlay(alignment: .topLeading) {
                if hovering, isEnabled {
                    bubble()
                        .padding(.horizontal, 12)
                        .padding(.vertical, 9)
                        .frame(width: width, alignment: .leading)
                        .background(
                            themeStore.commandModePanelColor(),
                            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .stroke(themeStore.borderColor(), lineWidth: 1)
                        )
                        .shadow(color: .black.opacity(0.35), radius: 10, y: 4)
                        .fixedSize()
                        .frame(width: 0, height: 0, alignment: .bottomLeading)
                        .offset(y: -8)
                        .allowsHitTesting(false)
                }
            }
    }
}

extension View {
    /// Shows a click-through bubble with `content` above this view while
    /// it is hovered.
    /// - Parameters:
    ///   - isEnabled: Gate for the bubble; pass false to suppress it
    ///     (e.g. when there is nothing to show).
    ///   - width: Fixed bubble width; text wraps to fit. Nil sizes the
    ///     bubble to its content.
    func hoverBubble<Content: View>(
        isEnabled: Bool = true,
        width: CGFloat? = nil,
        @ViewBuilder content: @escaping () -> Content
    ) -> some View {
        modifier(HoverBubbleModifier(isEnabled: isEnabled, width: width, bubble: content))
    }
}
