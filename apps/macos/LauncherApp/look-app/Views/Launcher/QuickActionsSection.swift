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
    var onActivateItem: (QuickActionDescriptor, QuickActionListItem) -> Void = { _, _ in }

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
                    onActivateItem: { item in onActivateItem(descriptor, item) }
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
    let onActivateItem: (QuickActionListItem) -> Void

    private enum Layout {
        static let sectionSpacing: CGFloat = 4
        static let contentSpacing: CGFloat = 10
        static let controlSpacing: CGFloat = 8
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 8
        static let cornerRadius: CGFloat = 8
        static let rowBackgroundOpacity = 0.18
        static let toggleKeyHint = "⌘O"
        static let hintFontSizeDelta: CGFloat = 3
        static let minHintFontSize: CGFloat = 10
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
                    DeviceRow(item: item, hintFont: hintFont, themeStore: themeStore) {
                        onActivateItem(item)
                    }
                }
            }
            .padding(.top, Layout.listTopPadding)
        }
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

/// One paired-device row: a connection dot, the name, and a "Connected" marker.
/// Clickable to connect/disconnect, with a hover highlight so it reads as active.
private struct DeviceRow: View {
    let item: QuickActionListItem
    let hintFont: Font
    let themeStore: ThemeStore
    let onActivate: () -> Void

    @State private var hovering = false

    private enum Layout {
        static let spacing: CGFloat = 8
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 4
        static let cornerRadius: CGFloat = 8
        static let dotSize: CGFloat = 7
        static let restOpacity = 0.10
        static let hoverOpacity = 0.28
    }

    var body: some View {
        Button(action: onActivate) {
            HStack(spacing: Layout.spacing) {
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
            .padding(.vertical, Layout.verticalPadding)
            .background(
                themeStore.dividerColor().opacity(hovering ? Layout.hoverOpacity : Layout.restOpacity),
                in: RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(item.id == nil)
        .onHover { inside in
            hovering = inside
            if item.id != nil {
                inside ? NSCursor.pointingHand.push() : NSCursor.pop()
            }
        }
    }
}
