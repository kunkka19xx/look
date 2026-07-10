import AppKit
import SwiftUI

// Folder-browse: arrow-key navigation of the preview pane's folder listing.
// The classic two-pane layout stays as-is; only the selection moves.
//
//   Right   - with a folder result selected, jump the selection into the
//             right-hand preview listing; on a highlighted subfolder, drill
//             into it (the preview pane re-lists that subfolder)
//   Up/Down - move the highlight through the preview rows
//   Left    - back up one level; at the first folder, return the selection
//             to the results list
//   Enter   - open the highlighted child (file in its default app, folder
//             in Finder)
//   Esc     - return the selection to the results list
//
// Typing continues to edit the query as usual, which exits browse and runs
// a fresh search.
extension LauncherView {
    var isFolderBrowseMode: Bool { !browseStack.isEmpty }

    var currentBrowsePath: String? { browseStack.last }

    /// Synthetic preview-pane subject while drilled below the selected
    /// result's own folder, so the header/path/modified rows describe the
    /// folder actually being listed. At depth 1 the real selected result is
    /// previewed, exactly as before.
    var browsePreviewResult: LauncherResult? {
        guard browseStack.count > 1, let path = browseStack.last else { return nil }
        return LauncherResult(
            id: AppConstants.Launcher.FolderBrowse.resultIDPrefix + path,
            kind: .folder,
            title: URL(fileURLWithPath: path).lastPathComponent,
            subtitle: path,
            path: path,
            score: 0
        )
    }

    /// Up/Down while the preview selection is active. Returns true when the
    /// key was consumed; false hands it back to the results list.
    func handleBrowseArrowDown() -> Bool {
        guard isFolderBrowseMode else { return false }
        stepBrowseSelection(by: 1)
        return true
    }

    func handleBrowseArrowUp() -> Bool {
        guard isFolderBrowseMode else { return false }
        stepBrowseSelection(by: -1)
        return true
    }

    private func stepBrowseSelection(by delta: Int) {
        guard let count = browseListing?.items.count, count > 0 else { return }
        browseIndex = FolderBrowseLogic.steppedIndex(from: browseIndex, count: count, delta: delta)
    }

    /// Right arrow. Returns true when consumed; false lets the event fall
    /// through to the text field so the caret still moves while editing.
    func handleBrowseArrowRight() -> Bool {
        if isFolderBrowseMode {
            guard let currentBrowsePath,
                let listing = browseListing,
                listing.items.indices.contains(browseIndex)
            else { return true }
            let entry = listing.items[browseIndex]
            // Right on a file is a no-op (still consumed - the selection
            // lives in the preview, not the text field).
            guard entry.isDir else { return true }
            browseInto(path: FolderBrowseLogic.childPath(parent: currentBrowsePath, name: entry.name))
            return true
        }

        guard !isCommandMode, !appUIState.showsThemeSettings, !showsHelpScreen,
            !isPrefixSuggestionQuery, !isCommandSuggestionQuery, !isClipboardQuery,
            pendingKillCandidate == nil, pendingEmptyTrashCount == nil
        else { return false }
        guard isQueryCaretAtEndOrTextEmpty else { return false }
        guard let selectedResultID,
            let selected = displayedResults.first(where: { $0.id == selectedResultID }),
            selected.kind == .folder,
            !DeleteTargetLogic.isURLScheme(selected.path),
            // ~/.Trash is TCC-protected and gets a Finder-backed summary
            // instead of a listing - nothing to browse into.
            !DeleteTargetLogic.isTrashPath(selected.path, homeDirectory: NSHomeDirectory())
        else { return false }
        guard FileManager.default.fileExists(atPath: selected.path) else {
            showBanner("This folder no longer exists", style: .error, duration: 1.4)
            return true
        }
        browseInto(path: selected.path)
        return true
    }

    /// Left arrow. Returns true when consumed (stepped up / exited browse).
    func handleBrowseArrowLeft() -> Bool {
        guard isFolderBrowseMode else { return false }
        browseBackToParent()
        return true
    }

    func browseInto(path: String) {
        browseStack.append(path)
        loadBrowseListing(for: path, selecting: nil)
    }

    func browseBackToParent() {
        guard let leavingPath = browseStack.popLast() else { return }
        guard let parentPath = browseStack.last else {
            exitFolderBrowse()
            return
        }
        // Re-highlight the folder we just came out of so Left undoes Right.
        loadBrowseListing(for: parentPath, selecting: leavingPath)
    }

    /// Returns the selection to the results list.
    func exitFolderBrowse() {
        browseLoadTask?.cancel()
        browseLoadTask = nil
        browseStack = []
        browseListing = nil
        browseIndex = 0
    }

    /// Window-hide teardown: next open starts with the selection in the
    /// results list as usual.
    func resetFolderBrowseStateOnHide() {
        exitFolderBrowse()
    }

    /// Enter while the preview selection is active: open the highlighted
    /// child. Browse rows aren't indexed candidates, so no usage is recorded
    /// (same rule as quick-folder rows).
    func openPreviewSelectionIfActive() -> Bool {
        guard isFolderBrowseMode else { return false }
        guard let currentBrowsePath,
            let listing = browseListing,
            listing.items.indices.contains(browseIndex)
        else { return true }
        let entry = listing.items[browseIndex]
        let childPath = FolderBrowseLogic.childPath(parent: currentBrowsePath, name: entry.name)
        guard FileManager.default.fileExists(atPath: childPath) else {
            showBanner("This item no longer exists", style: .error, duration: 1.4)
            return true
        }
        openTargetAsync(childPath)
        hideLauncherWindow(restorePreviousApp: false)
        return true
    }

    func loadBrowseListing(for path: String, selecting selectPath: String?) {
        browseLoadTask?.cancel()
        browseListing = nil
        browseIndex = 0
        browseLoadTask = Task {
            let listing = await FolderListingService.list(path: path)
            guard !Task.isCancelled, browseStack.last == path else { return }

            guard let listing else {
                // Unreadable folder (permissions, TCC): surface it and step
                // back to wherever we came from.
                showBanner("Cannot read this folder", style: .error, duration: 1.6)
                browseBackToParent()
                return
            }

            browseListing = listing
            if let selectPath,
                let index = listing.items.firstIndex(where: {
                    FolderBrowseLogic.childPath(parent: path, name: $0.name) == selectPath
                }) {
                browseIndex = index
            }
        }
    }

    /// True when the caret sits at the end of the query text (or the field is
    /// empty / not being edited), i.e. a Right arrow would be a no-op for text
    /// editing and is safe to repurpose for folder navigation.
    private var isQueryCaretAtEndOrTextEmpty: Bool {
        if query.isEmpty { return true }
        guard let editor = NSApp.keyWindow?.firstResponder as? NSTextView else { return true }
        let range = editor.selectedRange()
        return range.length == 0 && range.location >= (editor.string as NSString).length
    }
}
