import SwiftUI

struct SearchInputBar: View {
    @Binding var text: String
    @Binding var isCommandMode: Bool
    let isQueryFocused: FocusState<Bool>.Binding
    let activeCommand: AppCommand?
    let themeStore: ThemeStore
    let onSubmit: () -> Void
    let onExitCommandMode: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: isCommandMode ? "terminal" : "magnifyingglass")
                .foregroundStyle(isCommandMode ? themeStore.accentColor() : themeStore.secondaryTextColor())
            TextField(
                isCommandMode
                    ? (activeCommand?.placeholder ?? "Choose a command with Tab")
                    : "Search apps",
                text: $text
            )
                .textFieldStyle(.plain)
                .focused(isQueryFocused)
                .onTapGesture {
                    DispatchQueue.main.async {
                        isQueryFocused.wrappedValue = true
                    }
                }
                .onSubmit(onSubmit)

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
        .padding(.vertical, 10)
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
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
            LazyVStack(spacing: 4) {
                ForEach(commands) { command in
                    HStack(spacing: 8) {
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
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .background(
                        (selectedID == command.id || activeID == command.id)
                            ? themeStore.selectionFillColor() : themeStore.controlFillColor().opacity(0.75),
                        in: RoundedRectangle(cornerRadius: 6, style: .continuous)
                    )
                    .onTapGesture { onSelect(command.id) }
                }
            }
            .padding(2)
        }
        .padding(6)
        .background(themeStore.panelFillColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
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

            TextField(command.placeholder, text: $text)
                .textFieldStyle(.plain)
                .focused(isQueryFocused)
                .onSubmit(onSubmit)

            Text("/\(command.id)")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(themeStore.selectionFillColor(), in: Capsule())
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
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
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

struct ResultsListView: View {
    let results: [LauncherResult]
    let selectedID: String?
    let themeStore: ThemeStore
    let onSelect: (String) -> Void
    let onOpen: (String) -> Void

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 4) {
                    ForEach(results) { result in
                        LauncherRowView(
                            result: result,
                            isSelected: selectedID == result.id,
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
                withAnimation(.easeOut(duration: 0.12)) {
                    proxy.scrollTo(newID, anchor: .center)
                }
            }
        }
    }
}

struct HintBar: View {
    let hint: String
    let themeStore: ThemeStore

    var body: some View {
        Text(hint)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
            .foregroundStyle(themeStore.secondaryTextColor())
    }
}

struct ClipboardEmptyStateView: View {
    let themeStore: ThemeStore

    var body: some View {
        HStack(spacing: 0) {
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

            Rectangle()
                .fill(themeStore.dividerColor())
                .frame(width: 1)
                .padding(.vertical, 4)

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
}

struct LauncherHelpScreenView: View {
    let themeStore: ThemeStore

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Text(LauncherHelpContent.title)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 3), weight: .semibold))
                    Spacer()
                    Text(LauncherHelpContent.closeHint)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                }

                Text(LauncherHelpContent.subtitle)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())

                ShortcutHelpSection(title: "Main", items: LauncherHelpContent.mainShortcuts)
                ShortcutHelpSection(title: "Query prefixes", items: LauncherHelpContent.queryModes)
                ShortcutHelpSection(title: "Command mode", items: LauncherHelpContent.commandMode)
            }
            .padding(12)
        }
    }
}

private enum LauncherHelpContent {
    static let title = "Keyboard Help"
    static let closeHint = "Cmd+H to close"
    static let subtitle = "Quick guide for app list, clipboard search, and command flow."

    static let mainShortcuts: [(String, String)] = [
        ("Enter", "Open selected app/file/folder or copy selected clipboard item"),
        ("Cmd+C", "Copy selected file/folder to pasteboard"),
        ("Tab / Shift+Tab", "Move selection"),
        ("Up / Down", "Move selection"),
        ("Cmd+F", "Reveal selected app/file/folder in Finder"),
        ("Cmd+Enter", "Search current query on Google"),
        ("Cmd+/", "Enter command mode"),
        ("Cmd+Shift+,", "Open/close settings panel"),
        ("Cmd+Shift+;", "Reload .look.config"),
        ("Cmd+H", "Toggle this help screen"),
        ("Esc", "Close help / back / hide launcher"),
    ]

    static let queryModes: [(String, String)] = [
        ("a\"word", "Apps only"),
        ("f\"word", "Files only"),
        ("d\"word", "Folders only"),
        ("r\"pattern", "Regex search"),
        ("c\"word", "Clipboard history search (latest 10 text clips)"),
        ("t\"word", "Web translate (VI/EN/JA)"),
        ("tw\"word", "Lookup panel with definitions"),
    ]

    static let commandMode: [(String, String)] = [
        ("Tab / Shift+Tab", "Switch command"),
        ("Cmd+1 / Cmd+2 / Cmd+3 / Cmd+4", "Switch command"),
        (":3000", "Find process listening on port"),
        ("Up / Down", "Select app in kill results"),
        ("Y / N", "Confirm/cancel kill action"),
    ]
}

private struct ShortcutHelpSection: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let title: String
    let items: [(String, String)]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())

            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(item.0)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(themeStore.controlFillColor(), in: Capsule())
                    Text(item.1)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                    Spacer(minLength: 0)
                }
            }
        }
    }
}
