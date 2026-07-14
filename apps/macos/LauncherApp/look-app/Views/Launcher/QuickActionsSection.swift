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
    /// Item ids currently applying (connecting/disconnecting), rendered as busy.
    let pendingItems: Set<String>
    /// Actions with something applying. Their other controls go inert, so nothing looks
    /// clickable that the guard in `runQuickAction` would silently swallow.
    let busyActionIds: Set<String>
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
                    pendingItems: pendingItems,
                    isBusy: busyActionIds.contains(descriptor.actionId),
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
    let pendingItems: Set<String>
    /// Something under this action is applying, so its controls stop taking input.
    let isBusy: Bool
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
        /// Matches the pending row's dim, and the linows `.is-busy` rule.
        static let busyOpacity = 0.5
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
            Group {
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
            // While one of this action's devices is connecting, its own control stops
            // taking input rather than looking live and being swallowed by the guard.
            // `ToggleSwitch` already dims itself for an unresolved state, so only dim
            // here when there is a resolved state to dim.
            .disabled(isBusy)
            .opacity(isBusy && isOn != nil ? Layout.busyOpacity : 1)
        }
    }

    // MARK: - Info fields

    @ViewBuilder
    private var infoFields: some View {
        ForEach(descriptor.info, id: \.valueKey) { field in
            if let value = info[field.valueKey] {
                infoField(field, value)
            }
        }
    }

    @ViewBuilder
    private func infoField(_ field: QuickActionInfoField, _ value: InfoValue) -> some View {
        switch value {
        case .text(let text):
            statusRow(label: field.label, value: text)
        case .unavailable(let reason):
            statusRow(label: field.label, value: reason)
        case .list(let items):
            // A summary line (e.g. "Status: N connected"), then one row per item.
            VStack(alignment: .leading, spacing: Layout.itemSpacing) {
                statusRow(label: field.label, value: listSummary(items))
                ForEach(items, id: \.self) { item in
                    let isPending = item.id.map(pendingItems.contains) ?? false
                    ListItemRow(
                        item: item,
                        isPending: isPending,
                        // A sibling row is inert while another device applies. The
                        // applying row keeps its own pending styling.
                        isBusy: isBusy && !isPending,
                        hintFont: hintFont,
                        themeStore: themeStore
                    ) {
                        onActivateItem(item)
                    }
                }
            }
            .padding(.top, Layout.listTopPadding)
        }
    }

    /// A label/value line matching `InfoRow` (Kind, Path), used for the status.
    private func statusRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(infoFont)
                .foregroundStyle(themeStore.mutedTextColor())
            Spacer(minLength: 0)
            Text(value)
                .font(infoFont)
                .foregroundStyle(themeStore.secondaryTextColor())
                .lineLimit(1)
        }
        .padding(.horizontal, Layout.horizontalPadding)
    }

    /// Summary shown next to a list's label. A connectivity list (items carry an
    /// on/off marker) reports how many are on; a plain list just reports its size.
    private func listSummary(_ items: [QuickActionListItem]) -> String {
        guard items.contains(where: { $0.on != nil }) else {
            return "\(items.count)"
        }
        let connected = items.filter { $0.on == true }.count
        return "\(connected) connected"
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

    /// Label/value font for info rows, matching `InfoRow` (Kind, Path, Status).
    private var infoFont: Font {
        themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular)
    }

    private func keyHint(_ text: String) -> some View {
        Text(text)
            .font(themeStore.uiFont(size: hintFontSize, weight: .semibold))
            .foregroundStyle(themeStore.mutedTextColor())
    }
}

/// One actionable list item: a state dot, its label, clickable to act on it
/// (e.g. connect/disconnect a paired device), with a hover highlight. Generic -
/// any control whose info resolves to a `.list` renders its items through this.
private struct ListItemRow: View {
    let item: QuickActionListItem
    let isPending: Bool
    /// Another control under the same action is applying, so this row goes inert.
    let isBusy: Bool
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
        static let pendingOpacity = 0.5
    }

    /// Clickable only when it has an id, isn't already applying, and no sibling control
    /// under the same action is applying either.
    private var isActionable: Bool { item.id != nil && !isPending && !isBusy }

    var body: some View {
        Button(action: onActivate) {
            HStack(spacing: Layout.spacing) {
                // Filled = on, hollow = off, invisible = no on/off (plain item).
                // The clear placeholder keeps labels aligned across a mixed list.
                Circle()
                    .fill(item.on == true ? themeStore.fontColor() : Color.clear)
                    .overlay(Circle().strokeBorder(themeStore.mutedTextColor(), lineWidth: item.on == false ? 1 : 0))
                    .frame(width: Layout.dotSize, height: Layout.dotSize)
                Text(item.label)
                    .font(hintFont)
                    .foregroundStyle(themeStore.fontColor())
                    .lineLimit(1)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Layout.horizontalPadding)
            .padding(.vertical, Layout.verticalPadding)
            .background(
                themeStore.dividerColor().opacity(hovering && isActionable ? Layout.hoverOpacity : Layout.restOpacity),
                in: RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!isActionable)
        .opacity(isPending || isBusy ? Layout.pendingOpacity : 1)
        // Native pointer (macOS 15+) avoids the unbalanced NSCursor push/pop
        // stack that would leave a stuck cursor when a row is removed on reload.
        .pointerStyle(isActionable ? .link : nil)
        .onHover { hovering = $0 }
    }
}
