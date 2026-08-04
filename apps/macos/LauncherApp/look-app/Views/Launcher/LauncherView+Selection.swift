import AppKit
import SwiftUI

extension LauncherView {
    func setInitialSelection() {
        if isCommandMode {
            if let activeCommandID {
                selectedCommandID = activeCommandID
            } else {
                selectedCommandID = filteredCommands.first?.id
            }
        } else if hidesResultsForEmptyQuery {
            // No rows on screen. A seeded selection here is one the user cannot
            // see, and Enter / Cmd+D / Cmd+Shift+H would still act on it.
            selectedResultID = nil
        } else {
            selectedResultID = displayedResults.first?.id
        }
    }

    func moveSelection(
        _ direction: MoveCommandDirection,
        shouldAutocompleteCommand: Bool = false,
        preferCommandListInCommandMode: Bool = false
    ) {
        guard !appUIState.showsThemeSettings else { return }

        if isCommandMode
            && activeCommandID == AppConstants.Launcher.Command.kill
            && !preferCommandListInCommandMode
        {
            let suggestions = killSuggestions.prefix(20)
            guard !suggestions.isEmpty else { return }

            let currentNum = selectedKillSuggestionIndex
            let currentIndex = suggestions.firstIndex { $0.number == currentNum }

            let nextIndex: Int
            switch direction {
            case .down:
                if let currentIndex {
                    nextIndex = min(currentIndex + 1, suggestions.count - 1)
                } else {
                    nextIndex = 0
                }
            case .up:
                if let currentIndex {
                    nextIndex = max(currentIndex - 1, 0)
                } else {
                    nextIndex = suggestions.count - 1
                }
            default:
                return
            }

            selectedKillSuggestionIndex = suggestions[nextIndex].number
            return
        }

        if isCommandMode {
            guard !filteredCommands.isEmpty else {
                selectedCommandID = nil
                return
            }

            guard let currentID = selectedCommandID,
                let currentIndex = filteredCommands.firstIndex(where: { $0.id == currentID })
            else {
                selectedCommandID = filteredCommands.first?.id
                if shouldAutocompleteCommand {
                    autocompleteSelectedCommand()
                }
                return
            }

            let nextIndex: Int
            switch direction {
            case .down:
                nextIndex = (currentIndex + 1) % filteredCommands.count
            case .up:
                nextIndex = (currentIndex - 1 + filteredCommands.count) % filteredCommands.count
            default:
                return
            }

            selectedCommandID = filteredCommands[nextIndex].id
            if shouldAutocompleteCommand {
                autocompleteSelectedCommand()
            }
            return
        }

        guard !displayedResults.isEmpty else {
            selectedResultID = nil
            return
        }

        guard let currentID = selectedResultID,
            let currentIndex = displayedResults.firstIndex(where: { $0.id == currentID })
        else {
            selectedResultID = displayedResults.first?.id
            return
        }

        let nextIndex: Int
        switch direction {
        case .down:
            nextIndex = (currentIndex + 1) % displayedResults.count
        case .up:
            nextIndex = (currentIndex - 1 + displayedResults.count) % displayedResults.count
        default:
            return
        }

        // Wrapped so the selection pill glides between rows on arrow-key moves.
        // Click selection and results refreshes assign outside this animation, so
        // the pill snaps rather than sliding across unrelated rows.
        withAnimation(Motion.Selection.glide) {
            selectedResultID = displayedResults[nextIndex].id
        }
    }

    func autocompleteSelectedCommand() {
        guard isCommandMode,
            let commandID = selectedCommandID,
            filteredCommands.contains(where: { $0.id == commandID })
        else { return }

        activeCommandID = commandID
        commandFeedback = "Selected /\(commandID)"

        requestCommandInputFocusIfNeeded()
    }

    func startKeyboardNavigationIfNeeded() {
        guard !appUIState.showsThemeSettings else { return }
        keyboardMonitor.start(
            onNext: {
                moveSelection(.down, shouldAutocompleteCommand: true, preferCommandListInCommandMode: true)
            },
            onPrevious: {
                moveSelection(.up, shouldAutocompleteCommand: true, preferCommandListInCommandMode: true)
            },
            onArrowDown: {
                if isCommandMode {
                    if activeCommandID == AppConstants.Launcher.Command.kill {
                        moveSelection(.down)
                    }
                } else {
                    moveSelection(.down)
                }
            },
            onArrowUp: {
                if isCommandMode {
                    if activeCommandID == AppConstants.Launcher.Command.kill {
                        moveSelection(.up)
                    }
                } else {
                    moveSelection(.up)
                }
            },
            onEnterCommandMode: {
                if !isCommandMode {
                    enterCommandMode()
                }
            },
            onExitCommandMode: {
                exitCommandMode()
            },
            onHideLauncher: {
                hideLauncherWindow()
            },
            inCommandMode: { isCommandMode },
            onWebSearch: {
                performWebSearchFromQuery()
            },
            onRevealInFinder: {
                revealSelectedInFinder()
            },
            onCopySelection: {
                copySelectedResultToPasteboard()
            },
            onTogglePick: {
                togglePickForSelectedResult()
            },
            onClearPicked: {
                clearAllPicked()
            },
            onOpenAllPicked: {
                openAllPicked()
            },
            hasPickedItems: { [self] in
                !pickedKeys.isEmpty
            },
            onToggleHelp: {
                toggleHelpScreen()
            },
            onDismissHelpIfVisible: {
                dismissHelpIfVisible()
            },
            onSelectCommandByIndex: { [self] index in
                guard index > 0 && index <= commandCatalog.count else { return }
                let command = commandCatalog[index - 1]
                pendingKillCandidate = nil
                selectedKillSuggestionIndex = nil
                activeCommandID = command.id
                selectedCommandID = command.id
                commandFeedback = "Selected /\(command.id)"
                requestCommandInputFocusIfNeeded()
            },
            onActivateRunningApp: { [self] key in
                activateRunningApp(forKey: key)
            },
            onConfirmKill: { [self] in
                if let pendingKillCandidate {
                    runKillCommand(candidate: pendingKillCandidate)
                    self.pendingKillCandidate = nil
                }
            },
            onCancelKill: { [self] in
                pendingKillCandidate = nil
            },
            killConfirmationActive: { [self] in
                pendingKillCandidate != nil
            },
            onRequestDelete: { [self] in
                guard DeleteTargetLogic.allowsKeyboardDelete(
                    showsThemeSettings: appUIState.showsThemeSettings,
                    showsHelpScreen: showsHelpScreen
                ) else { return }
                let selected = displayedResults.first { $0.id == selectedResultID }
                if DeleteTargetLogic.removesClipboardHistory(selected), let selected {
                    deleteClipboardResult(resultID: selected.id)
                    return
                }
                requestDeleteSelection()
            },
            onConfirmDelete: { [self] in
                confirmDeleteSelection()
            },
            onCancelDelete: { [self] in
                cancelDeleteSelection()
            },
            deleteConfirmationActive: { [self] in
                pendingEmptyTrashCount != nil
            },
            onConfirmHideApp: { [self] in
                confirmHideSelectedApp()
            },
            onCancelHideApp: { [self] in
                cancelHideSelectedApp()
            },
            hideAppConfirmationActive: { [self] in
                pendingHideAppResult != nil
            },
            onToggleQuickAction: { [self] in
                togglePrimaryQuickAction()
            },
            hasToggleQuickAction: { [self] in
                hasToggleQuickAction
            },
            isLaunchpadActive: { [self] in
                isLaunchpadActive
            },
            onLaunchpadMnemonic: { [self] character in
                handleLaunchpadMnemonic(character)
            },
            onLaunchpadEscape: { [self] in
                cancelLaunchpadConfirmIfNeeded()
            },
            onHideSelectedApp: { [self] in
                hideSelectedApp()
            }
        )
    }
}
