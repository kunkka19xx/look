import SwiftUI

/// The actions portion of the info+actions panel (see docs/writing-controls.md).
/// A vertical stack of control components, one per declared action, rendered
/// beneath the result's info in `ResultPreviewView`. Each `ControlKind` maps to
/// its own building block (toggle switch, button, ...), so supporting a new
/// control type is adding a case in `QuickActionControl` - the rest of the panel
/// stays the same. This is the reusable template contributors compose against.
struct QuickActionsSection: View {
    let descriptors: [QuickActionDescriptor]
    let states: [String: ActionState]
    let themeStore: ThemeStore
    /// Called when a control is activated by click (Cmd+O runs the same path).
    var onRun: (QuickActionDescriptor, ActionIntent) -> Void = { _, _ in }

    private enum Layout {
        static let rowSpacing: CGFloat = 6
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Layout.rowSpacing) {
            ForEach(descriptors) { descriptor in
                QuickActionControl(
                    descriptor: descriptor,
                    state: states[descriptor.actionId],
                    themeStore: themeStore,
                    onRun: { intent in onRun(descriptor, intent) }
                )
            }
        }
    }
}

/// One action row: the title, the control for its kind, and its key hint.
private struct QuickActionControl: View {
    let descriptor: QuickActionDescriptor
    let state: ActionState?
    let themeStore: ThemeStore
    let onRun: (ActionIntent) -> Void

    private enum Layout {
        static let contentSpacing: CGFloat = 10
        static let controlSpacing: CGFloat = 8
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 8
        static let cornerRadius: CGFloat = 8
        static let rowBackgroundOpacity = 0.18
        static let toggleKeyHint = "⌘O"
        static let hintFontSizeDelta: CGFloat = 3
        static let minHintFontSize: CGFloat = 10
    }

    var body: some View {
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
