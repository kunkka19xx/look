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

        // The `@`-mention popup owns the highlight while it is open, so Tab
        // reaches into the file list instead of the results behind it.
        if direction == .down || direction == .up,
           moveMentionHighlight(forward: direction == .down) {
            return
        }

        // The picker owns Tab while it is up: it is the only thing on the
        // panel, and Enter is about to open one of its rows. Wrapped in the
        // shared curve, like every other list, or its pill would jump while the
        // rest glide.
        if direction == .down || direction == .up {
            var moved = false
            withAnimation(Motion.Selection.glide) {
                moved = actionController.movePickerSelection(forward: direction == .down)
            }
            if moved { return }
        }

        // Sessions list: Tab/Shift-Tab and ↑/↓ move the highlight over
        // [-1 = new chat, 0..<count = sessions]. Enter opens the highlighted
        // session, or starts a new chat when nothing is highlighted (-1).
        if isBrowsingConversations {
            let hi = filteredConversations.count - 1
            guard hi >= 0 else { return }
            var idx = selectedConversationIndex
            switch direction {
            // Wrap: past the last row loops back to the first, and vice versa.
            case .down: idx = (idx < 0 || idx >= hi) ? 0 : idx + 1
            case .up: idx = (idx <= 0) ? hi : idx - 1
            default: return
            }
            // Same curve as the results list, so the pill glides between rows.
            withAnimation(Motion.Selection.glide) {
                selectedConversationIndex = idx
            }
            return
        }

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
            onRecallPrompt: { [self] older in
                guard isAIMode else { return false }
                // Consumed even when it does nothing (empty history, or already
                // at the oldest/newest end), so a boundary press stays put
                // instead of reaching the composer as a move-by-paragraph.
                recallPrompt(older ? .up : .down)
                return true
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
            inAIMode: { isAIMode },
            onWebSearch: {
                performWebSearchFromQuery()
            },
            inActionMenu: { isActionMenuOpen },
            onToggleActionMenu: { toggleActionMenu() },
            onActionMenuMove: { moveActionMenuFocus(by: $0) },
            onActionMenuRun: { runFocusedAction() },
            onActionMenuClose: { closeActionMenu() },
            onRevealInFinder: {
                revealSelectedInFinder()
            },
            onEditSelection: {
                editSelectedResult()
            },
            onOpenTerminalForSelection: {
                openTerminalForSelectedResult()
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
            onActivateSession: { [self] index in
                openSessionAt(index)
            },
            onPopLevel: { [self] in
                popLevel()
            },
            onEscapeHome: { [self] in
                // Esc closes the file popup first and leaves the typed text
                // alone, so dismissing a suggestion never costs the message.
                // True means handled, which is what stops Esc also leaving AI
                // mode on the same press.
                if showsMentionPopup {
                    dismissMentionPopup()
                    return true
                }
                return exitAIToHome()
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
                // In AI mode ⌘D belongs to the sessions list and nothing else:
                // it deletes the highlighted conversation (same delete as ⌘⌫
                // and the row's trash button, undoable from the banner). With a
                // conversation open there is no delete target, and trashing a
                // file left selected in the main bar would be a nasty surprise,
                // so the chord stops here rather than falling through.
                if isAIMode {
                    if isBrowsingConversations {
                        deleteHighlightedSession()
                    }
                    return
                }
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
            onCancelAction: { [self] in
                // Esc ladder: a pending confirm cancels first (keep composing);
                // an open chat saves and drops to the sessions list (stay in AI
                // mode); the sessions list leaves AI mode for home.
                if actionController.isPresenting || actionController.awaitingChoice
                    || actionController.linkPicker != nil
                {
                    actionController.cancel()
                } else if !chat.sessionItems.isEmpty {
                    chat.endSession()
                    conversationCache = ConversationStore.load()
                    query = ""
                    selectedConversationIndex = -1
                } else {
                    chat.endSession()
                    query = ""
                    isAIMode = false
                }
            },
            actionConfirmationActive: { [self] in
                isActionSessionUI
            },
            onUndoAction: { [self] in
                // A just-deleted conversation is the most recent undoable thing.
                if undoConversationDelete() { return true }
                guard actionController.lastReceipt != nil else { return false }
                actionController.undoLast()
                return true
            },
            onStopGeneration: { [self] in
                guard chat.isStreamingAnswer else { return false }
                chat.stopGeneration()
                return true
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
