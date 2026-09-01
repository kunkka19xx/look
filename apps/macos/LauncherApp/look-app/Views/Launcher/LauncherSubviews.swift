import AppKit
import SwiftUI

struct SearchInputBar: View {
    @Binding var text: String
    @Binding var isCommandMode: Bool
    let isQueryFocused: FocusState<Bool>.Binding
    let activeCommand: AppCommand?
    let themeStore: ThemeStore
    /// AI mode (`>`): sparkles icon + its own placeholder, no prefix needed.
    var isAIMode: Bool = false
    /// When false the field draws no background of its own - used when it lives
    /// inside a shared top-row pane that already supplies one, so the search
    /// input and running-apps icons read as a single unified bar.
    var showsBackground: Bool = true
    /// Changes each time the launcher opens, replaying the spawn cascade.
    var revealToken: UInt64 = 0
    /// Where a drill-down is, shown as a leading chip.
    var breadcrumb: String?
    let onSubmit: () -> Void
    let onExitCommandMode: () -> Void

    private enum Layout {
        /// Matches where `NSTextField` starts drawing its own text, so the
        /// placeholder does not shift sideways as soon as you type.
        static let placeholderLeadingInset: CGFloat = 2
    }

    /// Command mode wins the bar's identity (see the icon below), so the badge
    /// steps aside rather than sitting next to the `/command` capsule.
    private var showsBetaBadge: Bool { isAIMode && !isCommandMode }

    private var placeholderText: String {
        if breadcrumb != nil {
            return "Filter, or Esc to go back"
        }
        if isCommandMode {
            return activeCommand?.placeholder ?? AppConstants.Launcher.commandModePlaceholder
        }
        if isAIMode {
            return "Ask, act, or search conversations"
        }
        return AppConstants.Launcher.searchPlaceholder
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(
                systemName: breadcrumb != nil
                    ? "chevron.right"
                    : (isCommandMode ? "terminal" : (isAIMode ? "sparkles" : "magnifyingglass"))
            )
            .foregroundStyle(
                isCommandMode || isAIMode || breadcrumb != nil
                    ? themeStore.accentColor() : themeStore.secondaryTextColor()
            )
            .contentTransition(.symbolEffect(.replace))
            .symbolEffect(.bounce, value: revealToken)

            if let breadcrumb {
                Text(breadcrumb)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1)))
                    .foregroundStyle(themeStore.fontColor())
                    .lineLimit(1)
                    .truncationMode(.head)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(themeStore.liftColor(opacity: 0.14), in: Capsule())
                    .accessibilityLabel("Inside \(breadcrumb)")
            }
            SmoothCaretTextField(
                text: $text,
                // Empty: the placeholder is drawn as the overlay below instead,
                // since an NSTextField's own placeholder cannot be animated.
                placeholder: "",
                isFocused: isQueryFocused,
                themeStore: themeStore,
                // Only the assistant composes prose; a search query with a line
                // break in it means nothing to the matcher.
                allowsMultiline: isAIMode,
                onSubmit: onSubmit
            )
                // The field's own placeholder is empty, so it would otherwise
                // reach VoiceOver unnamed.
                .accessibilityLabel(placeholderText)
                .frame(maxWidth: .infinity)
                .overlay(alignment: .leading) {
                    if text.isEmpty {
                        Text(placeholderText)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize)))
                            .foregroundStyle(themeStore.placeholderTextColor())
                            .lineLimit(1)
                            .padding(.leading, Layout.placeholderLeadingInset)
                            .allowsHitTesting(false)
                            // Decorative: the field above carries the name.
                            // `allowsHitTesting` does not remove it from the
                            // accessibility tree.
                            .accessibilityHidden(true)
                            .placeholderReveal(token: revealToken)
                    }
                }

            if showsBetaBadge {
                Text("BETA")
                    .font(
                        themeStore.uiFont(
                            size: CGFloat(max(9, themeStore.settings.fontSize - 4)),
                            weight: .semibold))
                    .foregroundStyle(themeStore.accentColor().opacity(0.9))
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(themeStore.liftColor(opacity: 0.14), in: Capsule())
                    .accessibilityLabel("AI is in beta")
            }

            if isCommandMode {
                if let command = activeCommand {
                    Text("/\(command.title)")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.fontColor())
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(themeStore.selectionFillColor(), in: Capsule())
                }
                Button("Exit") { onExitCommandMode() }
                    .keyboardShortcut(.escape, modifiers: [.shift])
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                    .buttonStyle(.plain)
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background {
            if showsBackground {
                RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous)
                    .fill(themeStore.controlFillColor())
            }
        }
    }
}

struct CommandFeedbackView: View {
    let message: String
    let themeStore: ThemeStore

    var body: some View {
        Text(message)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 4), weight: .semibold))
            .foregroundStyle(themeStore.fontColor())
            .lineLimit(30)
    }
}

struct CommandListView: View {
    let commands: [AppCommand]
    let selectedID: String?
    let activeID: String?
    let themeStore: ThemeStore
    let onSelect: (String) -> Void

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 3) {
                ForEach(commands) { command in
                    HStack(spacing: 6) {
                        Image(systemName: command.symbolName)
                            .frame(width: 18, height: 18)
                            .foregroundStyle(themeStore.accentColor())
                        VStack(alignment: .leading, spacing: 1) {
                            Text("/\(command.title)")
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                            Text(command.detail)
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.secondaryTextColor())
                                .lineLimit(1)
                        }
                        Spacer(minLength: 0)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 5)
                    .background(
                        (selectedID == command.id || activeID == command.id)
                            ? themeStore.selectionFillColor() : Color.clear,
                        in: RoundedRectangle(cornerRadius: themeStore.chipRadius, style: .continuous)
                    )
                    .onTapGesture { onSelect(command.id) }
                }
            }
            .padding(2)
        }
        .padding(5)
        // No outer panel-fill: matches the bg-less right column. Rows
        // sit directly on the command-mode backdrop. Only the selected /
        // active row paints a backdrop (selectionFillColor); other rows
        // are transparent.
        .frame(maxHeight: .infinity, alignment: .top)
    }
}

struct CommandInputBar: View {
    @Binding var text: String
    let command: AppCommand
    let isQueryFocused: FocusState<Bool>.Binding
    let themeStore: ThemeStore
    let onSubmit: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: command.symbolName)
                .foregroundStyle(themeStore.accentColor())

            SmoothCaretTextField(
                text: $text,
                placeholder: command.placeholder,
                isFocused: isQueryFocused,
                themeStore: themeStore,
                onSubmit: onSubmit
            )
                .frame(maxWidth: .infinity)

            Text("/\(command.id)")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(themeStore.selectionFillColor(), in: Capsule())
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous))
    }
}

struct CommandHeaderBar: View {
    let command: AppCommand
    let themeStore: ThemeStore
    let subtitle: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: command.symbolName)
                .foregroundStyle(themeStore.accentColor())

            Text(subtitle)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            Spacer(minLength: 0)

            Text("/\(command.id)")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(themeStore.selectionFillColor(), in: Capsule())
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous))
    }
}

struct ResultsListView: View {
    let results: [LauncherResult]
    let selectedID: String?
    let pickedKeys: Set<String>
    let themeStore: ThemeStore
    let onSelect: (String) -> Void
    let onOpen: (String) -> Void

    /// Backs the single selection pill that glides between rows.
    @Namespace private var selectionNamespace

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: false) {
                LazyVStack(spacing: 4) {
                    ForEach(results) { result in
                        LauncherRowView(
                            result: result,
                            isSelected: selectedID == result.id,
                            isPicked: pickedKeys.contains("\(result.kind.rawValue)|\(result.path)"),
                            isLast: result.id == results.last?.id,
                            selectionNamespace: selectionNamespace,
                            onOpen: {
                                onSelect(result.id)
                                onOpen(result.id)
                            }
                        )
                        .id(result.id)
                    }
                }
                .padding(2)
            }
            .onChange(of: selectedID) { _, newID in
                guard let newID else { return }
                // No anchor, so this scrolls the minimum needed to bring the row
                // into view and does nothing at all while the selection is
                // already visible. `.center` re-centred on every keypress, which
                // slid the whole list under a stationary pill and made the one
                // thing that actually moved the hardest thing to follow.
                //
                // Same curve as the pill, so the two stay together on the scrolls
                // that do happen.
                withAnimation(Motion.Selection.glide) {
                    proxy.scrollTo(newID)
                }
            }
        }
    }
}

struct PickedItemsPanel: View {
    let pickedKeys: [String]
    let pickedByKey: [String: LauncherResult]
    let themeStore: ThemeStore
    let onRemove: (String) -> Void
    let onClearAll: () -> Void
    let onOpenAll: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text("Picked (\(pickedKeys.count))")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                Spacer()
                Button(action: onOpenAll) {
                    HStack(spacing: 6) {
                        Text("Open all")
                        Text("⇧↵")
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: themeStore.microRadius, style: .continuous))
                            .foregroundStyle(themeStore.mutedTextColor())
                    }
                    .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                }
                .buttonStyle(.borderless)
                .foregroundStyle(themeStore.accentColor())
                Button(action: onClearAll) {
                    Text("Clear all")
                        .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                }
                .buttonStyle(.borderless)
                .foregroundStyle(themeStore.secondaryTextColor())
            }
            .padding(.horizontal, 10)
            .padding(.top, 8)

            ScrollView {
                LazyVStack(spacing: 4) {
                    ForEach(pickedKeys, id: \.self) { key in
                        if let r = pickedByKey[key] {
                            HStack(spacing: 8) {
                                Image(nsImage: NSWorkspace.shared.icon(forFile: r.path))
                                    .resizable()
                                    .frame(width: 18, height: 18)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(r.title)
                                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                                        .foregroundStyle(themeStore.fontColor())
                                        .lineLimit(1)
                                    Text(r.path)
                                        .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 4)), weight: .regular))
                                        .foregroundStyle(themeStore.mutedTextColor())
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                }
                                Spacer(minLength: 0)
                                Button(action: { onRemove(key) }) {
                                    Image(systemName: "xmark.circle.fill")
                                        .foregroundStyle(themeStore.mutedTextColor())
                                }
                                .buttonStyle(.borderless)
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                            .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: themeStore.chipRadius, style: .continuous))
                        }
                    }
                }
                .padding(.horizontal, 6)
                .padding(.bottom, 8)
            }
        }
        .frame(minWidth: 220)
    }
}

struct HintBar: View {
    /// Today's done/total quick view, clickable to open /todo. Shown on
    /// the home screen in place of the command-mode hint.
    struct TodoQuickView {
        let done: Int
        let total: Int
        /// Names of today's unfinished tasks, listed in the hover tooltip.
        let openTasks: [String]
        let onTap: () -> Void
    }

    let hint: String
    var todo: TodoQuickView? = nil
    let themeStore: ThemeStore

    private enum Layout {
        /// How far the hint may shrink before it truncates instead. Enough to
        /// absorb a long hint or a large font setting, not so much that the bar
        /// becomes unreadable.
        static let minimumScale: CGFloat = 0.75
    }

    private var hintFont: Font {
        themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular)
    }

    var body: some View {
        // One line, always. The hint is a reminder, not content: a second line
        // would reflow the whole bar as the selection changes, and a longer
        // font or another hint must shrink the text rather than move the layout.
        HStack(spacing: 0) {
            Text(hint)
                .font(hintFont)
                .foregroundStyle(themeStore.secondaryTextColor())
                .lineLimit(1)
                .minimumScaleFactor(Layout.minimumScale)
                .truncationMode(.tail)
                .layoutPriority(1)

            if let todo {
                Text("  •  ")
                    .font(hintFont)
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .lineLimit(1)
                    .fixedSize()
                Button(action: todo.onTap) {
                    HStack(spacing: 5) {
                        Image(systemName: "checklist")
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 3)))
                        Text("Todo \(todo.done)/\(todo.total)")
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                            .lineLimit(1)
                            .fixedSize()
                    }
                    .foregroundStyle(themeStore.accentColor())
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                // hoverBubble (not hoverTooltip): the popover would
                // swallow the first click; the bubble is click-through,
                // so tapping always opens /todo directly.
                .hoverBubble(isEnabled: !todo.openTasks.isEmpty, width: 240) {
                    openTasksBubbleContent(todo)
                }
            }
        }
    }

    private func openTasksBubbleContent(_ todo: TodoQuickView) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Unfinished today")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .semibold))
                .foregroundStyle(themeStore.mutedTextColor())
            ForEach(Array(todo.openTasks.enumerated()), id: \.offset) { _, name in
                Text("• \(name)")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2)))
                    .foregroundStyle(themeStore.fontColor())
                    .lineLimit(2)
            }
        }
    }
}

struct ClipboardEmptyStateView: View {
    let themeStore: ThemeStore

    var body: some View {
        HStack(spacing: 0) {
            ClipboardEmptyInfoView(themeStore: themeStore)

            Rectangle()
                .fill(themeStore.dividerColor())
                .frame(width: 1)
                .padding(.vertical, 4)

            ClipboardEmptyHelpView(themeStore: themeStore)
        }
    }
}

/// Left half of the clipboard empty state - split out so the launcher can render
/// it as its own card (matching the results list) when the panes are floating.
struct ClipboardEmptyInfoView: View {
    let themeStore: ThemeStore

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "doc.on.clipboard")
                    .foregroundStyle(themeStore.accentColor())
                Text("Clipboard History")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 1), weight: .semibold))
            }

            Text("No clipboard items yet")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                .foregroundStyle(themeStore.secondaryTextColor())

            Text("Copy any text, then search with c\"word to find it here.")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
                .lineLimit(2)

            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

/// Right half of the clipboard empty state (the "How to use" tips).
struct ClipboardEmptyHelpView: View {
    let themeStore: ThemeStore

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("How to use")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
            Text("• Type c\" to list latest 10 clips\n• Type c\"mail to filter\n• Press Enter to copy selected item")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
                .lineSpacing(4)
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

struct RecentEmptyStateView: View {
    let themeStore: ThemeStore

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "clock.arrow.circlepath")
                    .foregroundStyle(themeStore.accentColor())
                Text("Recent files & folders")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 1), weight: .semibold))
            }

            Text("Nothing recent yet")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                .foregroundStyle(themeStore.secondaryTextColor())

            Text("Open files/folders through Look, or download/create some - newest activity shows here. Type rc\"word to filter.")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
                .lineLimit(3)

            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

/// Which slice of the help the screen is showing. `all` keeps the original one
/// scroll; the rest narrow it, so arriving from a mode lands on that mode's keys
/// instead of a page the reader has to search.
/// The help screen's capsules: every `ShortcutTopic`, plus an "All" that shows
/// the whole catalog. Only the screen needs `all`, so it lives here rather than
/// in the catalog Settings also reads.
enum LauncherHelpTopic: CaseIterable, Identifiable {
    case all
    case topic(ShortcutTopic)

    static var allCases: [LauncherHelpTopic] { [.all] + ShortcutTopic.allCases.map(Self.topic) }

    var id: String {
        switch self {
        case .all: return "all"
        case .topic(let topic): return topic.id
        }
    }

    var label: String {
        switch self {
        case .all: return "All"
        case .topic(let topic): return topic.label
        }
    }

    var groups: [ShortcutGroup] {
        switch self {
        case .all: return ShortcutCatalog.groups
        case .topic(let topic): return ShortcutCatalog.groups(for: topic)
        }
    }

    static let ai = LauncherHelpTopic.topic(.ai)
}

extension LauncherHelpTopic: Equatable {
    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

struct LauncherHelpScreenView: View {
    private enum Metrics {
        static let selectedCapsuleOpacity = 0.22
        static let capsuleSpacing: CGFloat = 6
        static let capsuleHorizontalPadding: CGFloat = 10
        static let capsuleVerticalPadding: CGFloat = 4
        /// Wider than the gap between capsules, so the group reads as one unit
        /// next to the title rather than a sixth capsule.
        static let titleRowSpacing: CGFloat = 12
    }

    let themeStore: ThemeStore
    /// Where the screen opens. ⌘H from AI mode passes `.ai` so the assistant's
    /// keys are the first thing on screen.
    var initialTopic: LauncherHelpTopic = .all

    @State private var topic: LauncherHelpTopic

    init(themeStore: ThemeStore, initialTopic: LauncherHelpTopic = .all) {
        self.themeStore = themeStore
        self.initialTopic = initialTopic
        _topic = State(initialValue: initialTopic)
    }

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                // The topics ride in the title row rather than owning a band of
                // their own: they are navigation for this screen, and a full
                // row of them pushed the first shortcut below the fold.
                HStack(spacing: Metrics.titleRowSpacing) {
                    Text(LauncherHelpContent.title)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 3), weight: .semibold))
                        .fixedSize()
                    topicPicker
                    Spacer(minLength: 0)
                    Text(LauncherHelpContent.closeHint)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .fixedSize()
                }

                AppUpdateStatusView(themeStore: themeStore)

                Text(LauncherHelpContent.subtitle)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())

                ForEach(topic.groups) { group in
                    ShortcutGroupView(title: group.title, entries: group.entries)
                }
            }
            .padding(12)
        }
        // The screen is rebuilt on each open, but a reused instance would keep
        // the last topic and ignore where the reader came from.
        .onChange(of: initialTopic) { _, requested in topic = requested }
    }

    private var topicPicker: some View {
        HStack(spacing: Metrics.capsuleSpacing) {
            ForEach(LauncherHelpTopic.allCases) { candidate in
                let isSelected = candidate == topic
                Button { topic = candidate } label: {
                    Text(candidate.label)
                        .font(themeStore.uiFont(
                            size: CGFloat(themeStore.settings.fontSize - 1),
                            weight: isSelected ? .semibold : .regular))
                        .foregroundStyle(isSelected ? themeStore.fontColor() : themeStore.mutedTextColor())
                        .padding(.horizontal, Metrics.capsuleHorizontalPadding)
                        .padding(.vertical, Metrics.capsuleVerticalPadding)
                        .background(
                            isSelected
                                ? themeStore.accentColor().opacity(Metrics.selectedCapsuleOpacity)
                                : themeStore.controlFillColor(),
                            in: Capsule())
                }
                .buttonStyle(.plain)
                .help("Show \(candidate.label) shortcuts")
            }
        }
        // Sits between the title and the close hint, so the capsules keep their
        // own width instead of being squeezed by the row.
        .fixedSize()
    }
}

private enum LauncherHelpContent {
    static let title = "Help"
    static let closeHint = "Cmd+H to close"
    static let subtitle = "Quick guide for app list, clipboard search, and command flow."
}
