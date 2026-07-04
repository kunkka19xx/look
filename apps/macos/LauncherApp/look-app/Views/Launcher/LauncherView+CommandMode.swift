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
        // Reopen the last-visited command panel; fall back to /calc on
        // first run (or if the persisted id refers to a command no
        // longer in the catalog).
        let preferred = appUIState.lastCommandID ?? AppConstants.Launcher.Command.calc
        let resolved = commandCatalog.contains { $0.id == preferred }
            ? preferred
            : AppConstants.Launcher.Command.calc
        activeCommandID = resolved
        selectedCommandID = resolved
        focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
    }

    func enterCommandMode(commandID: String, prefilledInput: String) {
        showsHelpScreen = false
        query = ""
        isCommandMode = true
        commandFeedback = ""
        activeCommandID = commandID
        selectedCommandID = commandID
        commandInput = prefilledInput
        focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
    }

    // Detects `:cmdid<space>...` (live trigger) and bare `:cmdid` (submit-only
    // trigger). Returns nil if `input` doesn't begin with `:` or the id after
    // `:` (up to the first whitespace) isn't an exact match for a command in
    // the catalog. `hasSpace` distinguishes the two trigger paths.
    func extractInlineCommand(from input: String) -> (id: String, args: String, hasSpace: Bool)? {
        guard input.hasPrefix(":") else { return nil }
        let body = input.dropFirst()

        if let spaceIdx = body.firstIndex(where: { $0.isWhitespace }) {
            let id = String(body[..<spaceIdx]).lowercased()
            guard !id.isEmpty, commandCatalog.contains(where: { $0.id == id }) else { return nil }
            let args = String(body[body.index(after: spaceIdx)...])
            return (id, args, true)
        }

        let id = String(body).lowercased()
        guard !id.isEmpty, commandCatalog.contains(where: { $0.id == id }) else { return nil }
        return (id, "", false)
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
            if let cmd = extractInlineCommand(from: trimmed), !cmd.hasSpace {
                enterCommandMode(commandID: cmd.id, prefilledInput: "")
            } else if let translationCommand = extractTranslationQuery(from: trimmed) {
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
        // Fixed sidebar width — the launcher window is non-resizable
        // (minWidth 620) so the previous GeometryReader-driven formula
        // (`max(170, usableWidth * 0.25)`) always evaluated to 170 in
        // practice. GeometryReader inside a hidden-titlebar window
        // re-measures during AppKit layout passes (including window
        // drag), which caused visible flicker on this screen. Drop it.
        let splitSpacing: CGFloat = 8
        let dividerWidth: CGFloat = 1
        let leftWidth: CGFloat = 170

        // Hide the command sidebar while /pomo is in standby/idle mode
        // — keeps the user's focus on the clock + music card with no
        // distractions. Other commands keep the sidebar always visible.
        let hideSidebar = activeCommandID == AppConstants.Launcher.Command.pomo
            && PomoSharedState.shared.idle

        HStack(spacing: splitSpacing) {
                if !hideSidebar {
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
                }

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
                        } else if activeCommandID != AppConstants.Launcher.Command.pomo
                            && activeCommandID != AppConstants.Launcher.Command.todo {
                            // /pomo and /todo render their own header inside
                            // their panels, so skip the redundant outer one.
                            CommandHeaderBar(
                                command: activeCommand,
                                themeStore: themeStore,
                                subtitle: "Read-only command"
                            )
                        }
                    }

                    ZStack(alignment: .topLeading) {
                        // Command mode now has a solid opaque backdrop at
                        // the launcher level (themedBackground branches on
                        // isCommandMode), so the previous outer panel-fill
                        // RoundedRectangle isn't needed — its only purpose
                        // was layering a card over the visualEffect blur,
                        // which is no longer present here. Each command
                        // owns its own card-style backdrops where wanted.

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
                            SystemInfoView(themeStore: themeStore)
                                .padding(8)
                        } else if activeCommandID == AppConstants.Launcher.Command.pomo {
                            PomoView(themeStore: themeStore)
                                .padding(2)
                        } else if activeCommandID == AppConstants.Launcher.Command.todo {
                            TodoView(themeStore: themeStore)
                                .padding(2)
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
