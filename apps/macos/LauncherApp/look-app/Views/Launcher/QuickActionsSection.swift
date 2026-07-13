import SwiftUI

/// The actions portion of the info+actions panel (see docs/writing-controls.md).
/// A vertical stack of control components, one per declared action, rendered
/// beneath the result's info in `ResultPreviewView`. Each `ControlKind` maps to
/// its own building block (toggle switch, button, ...), and an action's info
/// fields render below it: a single line (`.text`) or one clickable row per item
/// (`.list`, e.g. paired Bluetooth devices). Supporting a new control type is
/// adding a case in `QuickActionControl` - the rest of the panel stays the same.
struct QuickActionsSection: View {
    let descriptors: [QuickActionDescriptor]
    let states: [String: ActionState]
    /// actionId -> valueKey -> resolved info value (device list, status, ...).
    let info: [String: [String: InfoValue]]
    let themeStore: ThemeStore
    /// A control was activated by click (Cmd+O runs the same path).
    var onRun: (QuickActionDescriptor, ActionIntent) -> Void = { _, _ in }
    /// A list item (e.g. a device row) was clicked to connect/disconnect.
    var onActivateItem: (QuickActionDescriptor, String) -> Void = { _, _ in }

    private enum Layout {
        static let rowSpacing: CGFloat = 6
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Layout.rowSpacing) {
            ForEach(descriptors) { descriptor in
                QuickActionControl(
                    descriptor: descriptor,
                    state: states[descriptor.actionId],
                    info: info[descriptor.actionId] ?? [:],
                    themeStore: themeStore,
                    onRun: { intent in onRun(descriptor, intent) },
                    onActivateItem: { itemId in onActivateItem(descriptor, itemId) }
                )
            }
        }
    }
}

/// One action: the control row (title + control + key hint) plus its info fields.
private struct QuickActionControl: View {
    let descriptor: QuickActionDescriptor
    let state: ActionState?
    let info: [String: InfoValue]
    let themeStore: ThemeStore
    let onRun: (ActionIntent) -> Void
    let onActivateItem: (String) -> Void

    private enum Layout {
        static let sectionSpacing: CGFloat = 4
        static let contentSpacing: CGFloat = 10
        static let controlSpacing: CGFloat = 8
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 8
        static let cornerRadius: CGFloat = 8
        static let rowBackgroundOpacity = 0.18
        static let itemBackgroundOpacity = 0.10
        static let toggleKeyHint = "⌘O"
        static let hintFontSizeDelta: CGFloat = 3
        static let minHintFontSize: CGFloat = 10
        static let dotSize: CGFloat = 7
        static let itemSpacing: CGFloat = 3
        static let listTopPadding: CGFloat = 2
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Layout.sectionSpacing) {
            controlRow
            infoFields
        }
    }

    private var controlRow: some View {
        HStack(spacing: Layout.contentSpacing) {
            Text(descriptor.title)
                .font(titleFont)
                .foregroundStyle(themeStore.fontColor())
            Spacer(minLength: 0)
            control
        }
        .padding(.horizontal, Layout.horizontalPadding)
        .padding(.vertical, Layout.verticalPadding)
        .background(
            themeStore.dividerColor().opacity(Layout.rowBackgroundOpacity),
            in: RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
        )
    }

    @ViewBuilder
    private var control: some View {
        if case .unavailable(let reason)? = state {
            Text(reason).font(hintFont).foregroundStyle(themeStore.mutedTextColor())
        } else {
            switch descriptor.control {
            case .toggle:
                HStack(spacing: Layout.controlSpacing) {
                    ToggleSwitch(isOn: isOn, themeStore: themeStore) { onRun(.toggle) }
                    keyHint(Layout.toggleKeyHint)
                }
            case .button:
                Button(descriptor.title) { onRun(.run) }
                    .buttonStyle(.borderless)
                    .font(hintFont)
            }
        }
    }

    // MARK: - Info fields

    @ViewBuilder
    private var infoFields: some View {
        ForEach(descriptor.info, id: \.valueKey) { field in
            if let value = info[field.valueKey] {
                infoField(value)
            }
        }
    }

    @ViewBuilder
    private func infoField(_ value: InfoValue) -> some View {
        switch value {
        case .text(let text):
            // The toggle already conveys plain On/Off; only show extra detail.
            if text != "On" && text != "Off" {
                Text(text)
                    .font(hintFont)
                    .foregroundStyle(themeStore.mutedTextColor())
                    .padding(.horizontal, Layout.horizontalPadding)
            }
        case .unavailable(let reason):
            Text(reason)
                .font(hintFont)
                .foregroundStyle(themeStore.mutedTextColor())
                .padding(.horizontal, Layout.horizontalPadding)
        case .list(let items):
            VStack(spacing: Layout.itemSpacing) {
                ForEach(items, id: \.self) { item in
                    deviceRow(item)
                }
            }
            .padding(.top, Layout.listTopPadding)
        }
    }

    private func deviceRow(_ item: QuickActionListItem) -> some View {
        Button {
            if let id = item.id { onActivateItem(id) }
        } label: {
            HStack(spacing: Layout.controlSpacing) {
                Circle()
                    .fill(item.on == true ? Color.green : Color.clear)
                    .overlay(Circle().strokeBorder(themeStore.mutedTextColor(), lineWidth: item.on == true ? 0 : 1))
                    .frame(width: Layout.dotSize, height: Layout.dotSize)
                Text(item.label)
                    .font(hintFont)
                    .foregroundStyle(themeStore.fontColor())
                    .lineLimit(1)
                Spacer(minLength: 0)
                if item.on == true {
                    Text("Connected")
                        .font(hintFont)
                        .foregroundStyle(themeStore.mutedTextColor())
                }
            }
            .padding(.horizontal, Layout.horizontalPadding)
            .padding(.vertical, Layout.verticalPadding / 2)
            .background(
                themeStore.dividerColor().opacity(Layout.itemBackgroundOpacity),
                in: RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
            )
        }
        .buttonStyle(.plain)
        .disabled(item.id == nil)
    }

    // MARK: - Helpers

    private var isOn: Bool? {
        switch state {
        case .on?: return true
        case .off?: return false
        case .value?, .unavailable?, nil: return nil
        }
    }

    private var titleFont: Font {
        themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium)
    }

    /// Size for hints and secondary labels: a few points below the base, floored.
    private var hintFontSize: CGFloat {
        max(Layout.minHintFontSize, CGFloat(themeStore.settings.fontSize) - Layout.hintFontSizeDelta)
    }

    private var hintFont: Font {
        themeStore.uiFont(size: hintFontSize, weight: .regular)
    }

    private func keyHint(_ text: String) -> some View {
        Text(text)
            .font(themeStore.uiFont(size: hintFontSize, weight: .semibold))
            .foregroundStyle(themeStore.mutedTextColor())
    }
}
