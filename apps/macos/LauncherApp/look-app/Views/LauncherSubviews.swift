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
                .foregroundStyle(isCommandMode ? .green : .secondary)
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
                        .background(.green.opacity(0.18), in: Capsule())
                }
                Button("Exit") { onExitCommandMode() }
                    .keyboardShortcut(.escape, modifiers: [.shift])
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                    .buttonStyle(.plain)
                    .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.72))
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(.white.opacity(0.14), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

struct ClipboardInputBar: View {
    @Binding var text: String
    let isQueryFocused: FocusState<Bool>.Binding
    let themeStore: ThemeStore
    let onSubmit: () -> Void
    let onExit: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "doc.on.clipboard")
                .foregroundStyle(.blue)
            TextField("Search clipboard history", text: $text)
                .textFieldStyle(.plain)
                .focused(isQueryFocused)
                .onTapGesture {
                    DispatchQueue.main.async {
                        isQueryFocused.wrappedValue = true
                    }
                }
                .onSubmit(onSubmit)

            Text("Clipboard")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(.blue.opacity(0.18), in: Capsule())

            Button("Exit") { onExit() }
                .keyboardShortcut(.escape, modifiers: [.shift])
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .buttonStyle(.plain)
                .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.72))
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(.white.opacity(0.14), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
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

    private var columns: [GridItem] {
        [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)]
    }

    var body: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 4) {
                ForEach(commands) { command in
                    HStack(spacing: 8) {
                        Image(systemName: "terminal")
                            .frame(width: 18, height: 18)
                            .foregroundStyle(.green)
                        VStack(alignment: .leading, spacing: 1) {
                            Text("/\(command.title)")
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                            Text(command.detail)
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.72))
                                .lineLimit(1)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .background(
                        (selectedID == command.id || activeID == command.id)
                            ? .green.opacity(0.20) : .white.opacity(0.08),
                        in: RoundedRectangle(cornerRadius: 6, style: .continuous)
                    )
                    .onTapGesture { onSelect(command.id) }
                }
            }
            .padding(2)
        }
        .frame(maxHeight: AppConstants.Launcher.commandListMaxHeight, alignment: .top)
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
    let isCommandMode: Bool
    var isClipboardMode: Bool = false
    let activeCommandID: String?
    let themeStore: ThemeStore

    var hint: String {
        if isClipboardMode {
            return AppConstants.Launcher.clipboardHint
        }
        if isCommandMode && activeCommandID == "kill" {
            return AppConstants.Launcher.killHint
        }
        if isCommandMode && activeCommandID == "sys" {
            return AppConstants.Launcher.sysHint
        }
        return isCommandMode ? AppConstants.Launcher.commandHint : AppConstants.Launcher.normalHint
    }

    var body: some View {
        Text(hint)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
            .foregroundStyle(themeStore.fontColor(opacityMultiplier: 0.72))
    }
}
