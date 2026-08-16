import SwiftUI

/// The list a `join` or `call` puts on the AI panel: rows to open, one of them
/// highlighted. Presentation only - which rows exist, and what opening one
/// does, belong to `ActionController+Links`.
extension LauncherView {
    /// The rows a `join` or `call` turned up. It always lists when there is
    /// any choice at all: the point is to see WHICH meeting, or WHICH way to
    /// reach someone, before a link opens. Tab/arrows move, Enter opens, a
    /// number picks directly.
    @ViewBuilder
    func linkPickerList(_ picker: ActionController.LinkPicker) -> some View {
        let fontSize = themeStore.settings.fontSize
        VStack(alignment: .leading, spacing: 6) {
            Text("\(picker.header)  ·  Tab to move  ·  Enter opens  ·  Esc cancels")
                .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .semibold))
                .foregroundStyle(themeStore.mutedTextColor())

            ForEach(Array(picker.options.enumerated()), id: \.element.id) { index, option in
                Button {
                    actionController.selectPickerRow(number: index + 1)
                    openHighlightedLink()
                } label: {
                    HStack(spacing: 10) {
                        Text("\(index + 1)")
                            .font(themeStore.uiFont(size: CGFloat(fontSize - 2), weight: .semibold))
                            .foregroundStyle(themeStore.accentColor())
                            .frame(minWidth: 14, alignment: .leading)
                        Image(systemName: option.symbol)
                            .font(.system(size: CGFloat(fontSize - 3)))
                            .foregroundStyle(themeStore.accentColor())
                        VStack(alignment: .leading, spacing: 1) {
                            Text(option.title)
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 1), weight: .medium))
                                .foregroundStyle(themeStore.fontColor())
                                .lineLimit(1)
                            // What the row is actually promising. Opening
                            // without showing this is what the first version
                            // got wrong.
                            Text(option.detail)
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                                .lineLimit(1)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background {
                        RoundedRectangle(cornerRadius: SelectionPill.Metrics.cornerRadius, style: .continuous)
                            .fill(themeStore.surfaceFill(0.55))
                    }
                    .selectionPill(
                        isSelected: index == picker.selected,
                        themeStore: themeStore,
                        namespace: linkPickerNamespace,
                        geometryID: Self.linkPickerPillID)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 4)
    }

    /// Its own pill id: two lists must never share one, or the pill flies
    /// between them.
    static let linkPickerPillID = "look.linkpicker.pill"
}
