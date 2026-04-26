import AppKit
import SwiftUI

extension LauncherView {
    func scheduleKillListRefresh() {
        killListRefreshTick &+= 1
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
            killListRefreshTick &+= 1
            if activeCommandID == AppConstants.Launcher.Command.kill {
                selectedKillSuggestionIndex = killSuggestions.first?.number
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            killListRefreshTick &+= 1
            if activeCommandID == AppConstants.Launcher.Command.kill {
                selectedKillSuggestionIndex = killSuggestions.first?.number
            }
        }
    }

    func requestCommandInputFocusIfNeeded() {
        guard isCommandMode else { return }
        guard activeCommandAcceptsInput else {
            isQueryFocused = false
            return
        }
        DispatchQueue.main.async {
            guard isCommandMode, activeCommandAcceptsInput else { return }
            focusActiveInput(recoveryDelays: [0.0], activateApp: false)
        }
    }

    func enterCommandMode() {
        showsHelpScreen = false
        isCommandMode = true
        commandInput = ""
        commandFeedback = ""
        activeCommandID = AppConstants.Launcher.Command.calc
        selectedCommandID = AppConstants.Launcher.Command.calc
        focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
    }

    func exitCommandMode() {
        guard isCommandMode else { return }
        isCommandMode = false
        commandInput = ""
        commandFeedback = ""
        activeCommandID = nil
        selectedCommandID = nil
        refreshSearchResults()
        focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
    }

    func handleSubmit() {
        logUIEvent("submit isCommand=\(isCommandMode) active=\(activeCommandID ?? "nil") selectedKill=\(selectedKillSuggestionIndex.map(String.init) ?? "nil") pendingKill=\(pendingKillCandidate?.displayName ?? "nil") input='\(commandArgsPart)'")
        if isCommandMode {
            if activeCommandID == AppConstants.Launcher.Command.kill, let selectedNum = selectedKillSuggestionIndex {
                if let candidate = killSuggestions.first(where: { $0.number == selectedNum }) {
                    pendingKillCandidate = candidate
                    logUIEvent("kill submit -> pending from selected index num=\(selectedNum) candidate=\(candidate.displayName) pid=\(candidate.pid)")
                } else {
                    selectedKillSuggestionIndex = nil
                    logUIEvent("kill submit -> stale selection num=\(selectedNum), fallback action")
                    runCommandModeAction()
                }
            } else {
                runCommandModeAction()
            }
        } else {
            let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
            if let translationCommand = extractTranslationQuery(from: trimmed) {
                handleTranslation(command: translationCommand)
                isQueryFocused = true
            } else {
                openSelectedApp()
            }
        }

        DispatchQueue.main.async {
            isQueryFocused = true
        }
    }

    func runCommandModeAction() {
        let resolvedCommand = commandCatalog.first(where: { $0.id == (activeCommandID ?? "") })
            ?? commandCatalog.first(where: { $0.id == commandNamePart.lowercased() })
            ?? commandCatalog.first(where: { $0.id == selectedCommandID })

        guard let resolvedCommand else {
            setCommandError("Unknown command. Try /shell, /calc, /kill, or /sys")
            return
        }

        switch resolvedCommand.id {
        case AppConstants.Launcher.Command.shell:
            guard !commandArgsPart.isEmpty else {
                setCommandError("Usage: /shell <command>")
                return
            }
            commandFeedback = "Running..."
            ShellCommand.run(commandArgsPart) { [self] message in
                commandFeedback = message
                isQueryFocused = true
            }
        case AppConstants.Launcher.Command.calc:
            guard !commandArgsPart.isEmpty else {
                setCommandError("Usage: /calc <expression>")
                return
            }
            let result = CalcCommand.evaluate(commandArgsPart)
            switch result {
            case .value(let value):
                commandFeedback = "Result: \(value)"
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(value, forType: .string)
            case .error(let message):
                setCommandError(message)
            }
        case AppConstants.Launcher.Command.kill:
            let searchTerm = commandArgsPart.trimmingCharacters(in: .whitespacesAndNewlines)
            let matched = KillCommand.suggestions(searchTerm: searchTerm)
            logUIEvent("kill action search='\(searchTerm)' matches=\(matched.count)")

            if matched.isEmpty {
                if searchTerm.hasPrefix(":") || searchTerm.lowercased().hasPrefix("port ") {
                    commandFeedback = "No process listening on this port"
                } else {
                    commandFeedback = "No matching apps. /kill to list all. Use :3000 to search by port."
                }
            } else if searchTerm.isEmpty {
                let appList = matched.map { candidate in
                    "\(candidate.number). \(candidate.displayName) (PID: \(candidate.pid))"
                }
                commandFeedback = "Running apps:\n" + appList.joined(separator: "\n") + "\n\n/kill <name or number>"
            } else if matched.count > 1 {
                let list = matched.map { candidate in "\(candidate.number). \(candidate.displayName)" }
                commandFeedback = "Multiple matches:\n" + list.joined(separator: "\n") + "\n\nBe more specific."
            } else {
                let candidate = matched[0]
                selectedKillSuggestionIndex = candidate.number
                pendingKillCandidate = candidate
                logUIEvent("kill action -> pending single candidate=\(candidate.displayName) pid=\(candidate.pid)")
            }
        case AppConstants.Launcher.Command.sys:
            commandFeedback = ""
        default:
            setCommandError("Unsupported command")
        }
    }

    func setCommandError(_ message: String) {
        commandFeedback = message
        showBanner(message)
    }

    func runKillCommand(candidate: KillCommand.Candidate) {
        logUIEvent("kill execute candidate=\(candidate.displayName) pid=\(candidate.pid)")
        KillCommand.kill(pid: candidate.pid, name: candidate.displayName) { [self] message in
            commandFeedback = message
            logUIEvent("kill completion message='\(message)'")
            pendingKillCandidate = nil
            selectedKillSuggestionIndex = nil

            if message.hasPrefix("Killed:") {
                recentlyKilledPIDs.insert(candidate.pid)
                DispatchQueue.main.asyncAfter(deadline: .now() + 10.0) {
                    recentlyKilledPIDs.remove(candidate.pid)
                }
            }
            scheduleKillListRefresh()
        }
    }

    func selectCommand(_ commandID: String) {
        pendingKillCandidate = nil
        selectedKillSuggestionIndex = nil
        if commandID != AppConstants.Launcher.Command.kill {
            recentlyKilledPIDs.removeAll()
        }
        activeCommandID = commandID
        selectedCommandID = commandID
        commandInput = ""
        commandFeedback = "Selected /\(commandID)"
        requestCommandInputFocusIfNeeded()
    }

    @ViewBuilder
    var commandModeView: some View {
        GeometryReader { proxy in
            let splitSpacing: CGFloat = 8
            let dividerWidth: CGFloat = 1
            let usableWidth = max(0, proxy.size.width - splitSpacing - dividerWidth)
            let leftWidth = max(170, usableWidth * 0.25)

            HStack(spacing: splitSpacing) {
                CommandListView(
                    commands: commandCatalog,
                    selectedID: selectedCommandID,
                    activeID: activeCommandID,
                    themeStore: themeStore,
                    onSelect: selectCommand
                )
                .frame(width: leftWidth)
                .frame(maxHeight: .infinity, alignment: .topLeading)

                Rectangle()
                    .fill(themeStore.dividerColor())
                    .frame(width: dividerWidth)
                    .padding(.vertical, 2)

                VStack(alignment: .leading, spacing: 6) {
                    if let activeCommand {
                        if activeCommandAcceptsInput {
                            CommandInputBar(
                                text: $commandInput,
                                command: activeCommand,
                                isQueryFocused: $isQueryFocused,
                                themeStore: themeStore,
                                onSubmit: handleSubmit
                            )
                        } else {
                            CommandHeaderBar(
                                command: activeCommand,
                                themeStore: themeStore,
                                subtitle: "Read-only command"
                            )
                        }
                    }

                    ZStack(alignment: .topLeading) {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(themeStore.panelFillColor())

                    if activeCommandID == AppConstants.Launcher.Command.kill {
                        let killSearchTerm = commandArgsPart.trimmingCharacters(in: .whitespacesAndNewlines)
                        let portQuery = killSearchTerm.hasPrefix(":") || killSearchTerm.lowercased().hasPrefix("port ")
                        let defaultKillEmptyMessage = portQuery
                            ? "No process listening on this port"
                            : "No matches. Type an app name or use :3000"

                        KillCommandView(
                            suggestions: Array(killSuggestions),
                            selectedIndex: selectedKillSuggestionIndex,
                            emptyMessage: commandFeedback.isEmpty ? defaultKillEmptyMessage : commandFeedback,
                            themeStore: themeStore,
                            onSelect: { candidate in
                                pendingKillCandidate = candidate
                                selectedKillSuggestionIndex = candidate.number
                            }
                        )
                            .onAppear {
                                if selectedKillSuggestionIndex == nil {
                                    selectedKillSuggestionIndex = killSuggestions.first?.number
                                }
                            }
                            .padding(8)
                        } else if activeCommandID == AppConstants.Launcher.Command.sys {
                            SystemInfoView(items: SystemInfoCommand.getSystemInfoItems(), themeStore: themeStore)
                                .padding(8)
                        } else {
                            VStack(alignment: .leading, spacing: 0) {
                                CommandFeedbackView(
                                    message: liveCommandPreview ?? (commandFeedback.isEmpty ? AppConstants.Launcher.commandEmptyMessage : commandFeedback),
                                    themeStore: themeStore
                                )
                                Spacer(minLength: 0)
                            }
                            .padding(10)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }
}
