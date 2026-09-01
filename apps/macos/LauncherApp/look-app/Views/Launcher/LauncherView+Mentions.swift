import SwiftUI

/// `@`-mentions in the AI input: fuzzy-find a file with look's own index, attach
/// it to the turn, and show it in a bar the user can see and empty.
///
/// The popup is PASSIVE. It never captures a keystroke and never inserts
/// anything on its own, so `>add lunch @1pm` still means a time to
/// `explicit.rs`. Tab reaches into the list, Enter accepts only once something
/// is highlighted, and Enter with nothing highlighted sends the message as it
/// always has.
extension LauncherView {
    /// The caret is assumed to be at the end: mentions are typed at the point
    /// of composition, and the field does not publish its selection.
    var mentionActive: MentionQuery.Active? {
        guard isAIMode, !isCommandMode else { return nil }
        return MentionQuery.active(in: query, caret: query.count)
    }

    var showsMentionPopup: Bool { !mentionMatches.isEmpty }

    /// Fuzzy file search through the same engine path `f"` uses, so a mention
    /// finds exactly what the launcher would.
    func refreshMentionMatches() {
        mentionSearchTask?.cancel()
        guard let active = mentionActive else {
            // Only publish when something actually changes: this runs on every
            // keystroke in every mode, and a no-op assignment still invalidates
            // the (large) launcher body.
            if !mentionMatches.isEmpty || mentionHighlight != -1 {
                mentionMatches = []
                mentionHighlight = -1
            }
            return
        }
        let token = active.token
        // Everything the detached search needs is captured HERE: the project
        // defaults to MainActor isolation, so reading these inside the hop
        // would be an async access, not a plain read.
        let bridge = EngineBridge.shared
        let searchQuery = AppConstants.Launcher.QueryPrefix.files + token
        let limit = Self.mentionLimit
        mentionSearchTask = Task { @MainActor in
            // Debounced like every other search path: without this, "@report"
            // runs six full index searches while the main result search is
            // running for the same keystrokes.
            try? await Task.sleep(nanoseconds: AppConstants.Launcher.searchDebounceNanoseconds)
            if Task.isCancelled { return }
            let found = await Task.detached(priority: .userInitiated) {
                bridge.search(query: searchQuery, limit: limit)
            }.value
            guard !Task.isCancelled, mentionActive?.token == token else { return }
            mentionMatches = found.filter { $0.kind == .file }
            // Nothing highlighted, so Enter keeps meaning "send".
            mentionHighlight = -1
        }
    }

    nonisolated static let mentionLimit = 6

    /// The mention list's own pill id. One per list: sharing an id with another
    /// list on screen would make the pill fly between the two.
    nonisolated static let mentionPillID = "look.mention.pill"

    /// Tab / Shift-Tab (and the arrows) roll the list. Returns false when there
    /// is no popup, so the caller falls through to normal selection.
    @discardableResult
    func moveMentionHighlight(forward: Bool) -> Bool {
        guard showsMentionPopup else { return false }
        let count = mentionMatches.count
        let next: Int
        if forward {
            next = mentionHighlight >= count - 1 ? 0 : mentionHighlight + 1
        } else {
            next = mentionHighlight <= 0 ? count - 1 : mentionHighlight - 1
        }
        // Same curve as the results and session lists: the animation is what
        // makes the shared pill glide instead of jump. Only keyboard moves are
        // wrapped - a click or a fresh search should snap.
        withAnimation(Motion.Selection.glide) {
            mentionHighlight = next
        }
        return true
    }

    /// Enter over a highlighted row. Returns false when nothing is highlighted,
    /// which is what keeps a bare Enter a send.
    @discardableResult
    func acceptHighlightedMention() -> Bool {
        guard showsMentionPopup,
              mentionHighlight >= 0, mentionHighlight < mentionMatches.count,
              let active = mentionActive
        else { return false }
        attach(path: mentionMatches[mentionHighlight].path)
        // Consuming the token leaves clean prose behind, so the submitted text
        // never carries a stray `@`.
        query = MentionQuery.consume(query, active).text
        dismissMentionPopup()
        return true
    }

    func attach(path: String) {
        guard let failure = attachments.add(path: path) else { return }
        let name = (path as NSString).lastPathComponent
        // Wording lives with the enum (`TextExtraction.Failure.message`); only
        // how loudly to say it is a view decision.
        let style: BannerStyle = failure == .unreadable ? .error : .info
        showBanner(failure.message(for: name), style: style, duration: 1.6)
    }

    func dismissMentionPopup() {
        mentionSearchTask?.cancel()
        mentionMatches = []
        mentionHighlight = -1
    }

    /// Empties the BAR only. The controller keeps the submitted set until the
    /// next submit overwrites it, because planning is async: clearing its copy
    /// here would leave the plan reading no attachments by the time it lands.
    func clearAttachments() {
        attachments.removeAll()
    }

    // MARK: - Views

    /// The attachment bar: what this turn is carrying, and how much of the
    /// model's window it will take. Above the popup so it never moves when the
    /// suggestion list opens and closes.
    @ViewBuilder
    var mentionAttachmentBar: some View {
        if !attachments.isEmpty {
            let fontSize = themeStore.settings.fontSize
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    ForEach(attachments.files) { file in
                        HStack(spacing: 4) {
                            Image(systemName: "doc.text")
                                .font(.system(size: CGFloat(fontSize - 4)))
                            Text(file.name)
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .medium))
                                .lineLimit(1)
                            Button {
                                attachments.remove(path: file.path)
                            } label: {
                                Image(systemName: "xmark")
                                    .font(.system(size: CGFloat(fontSize - 5)))
                            }
                            .buttonStyle(.plain)
                            // An icon-only button announces nothing, and there
                            // is one per attachment - so the name has to carry
                            // WHICH file it removes.
                            .accessibilityLabel("Remove \(file.name)")
                        }
                        .foregroundStyle(themeStore.fontColor())
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(
                            themeStore.accentColor().opacity(0.16),
                            in: Capsule())
                        .help(file.path)
                    }
                    Spacer(minLength: 0)
                }
                // Ollama truncates an over-long prompt silently, so say it here
                // rather than let a half-read file look like a bad answer.
                if attachments.exceedsContext(
                    AIQueryRouter.shared.contextTokens(of: themeStore.settings.aiProvider))
                {
                    Text(
                        "~\(attachments.estimatedTokens) tokens attached: more than this "
                            + "model's context will hold, so it may not see all of it."
                    )
                    .font(themeStore.uiFont(size: CGFloat(fontSize - 3), weight: .regular))
                    .foregroundStyle(themeStore.onDangerColor())
                    .padding(.horizontal, 7)
                    .padding(.vertical, 3)
                    .background(themeStore.dangerColor().opacity(0.85), in: Capsule())
                }
            }
            .padding(.horizontal, 4)
        }
    }

    private enum PopupMetrics {
        /// Tall enough that the preview shows real content, not a teaser: a
        /// source file needs a screenful before it identifies itself. Fixed, so
        /// the panel below does not jump as matches come and go.
        static let height: CGFloat = 380
        static let columnGap: CGFloat = 8
        static let dividerWidth: CGFloat = 1
    }

    /// The suggestion list beside a preview of the highlighted file. Two files
    /// with the same name is the normal case (six `main.go`s is a real result),
    /// so the path is on every row and the contents are one Tab away.
    @ViewBuilder
    var mentionPopup: some View {
        if showsMentionPopup {
            // Even halves: the paths and the contents are equally the thing the
            // reader is comparing, so neither column gets to be the sidebar.
            HStack(alignment: .top, spacing: PopupMetrics.columnGap) {
                mentionList
                    .frame(maxWidth: .infinity, alignment: .topLeading)

                Rectangle()
                    .fill(themeStore.dividerColor())
                    .frame(width: PopupMetrics.dividerWidth)

                mentionPreviewColumn
                    .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .frame(height: PopupMetrics.height)
            .padding(4)
            .background(
                themeStore.surfaceFill(0.92),
                in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous))
            .padding(.horizontal, 4)
        }
    }

    private var mentionList: some View {
        let fontSize = themeStore.settings.fontSize
        return VStack(alignment: .leading, spacing: 1) {
            ForEach(Array(mentionMatches.enumerated()), id: \.element.id) { index, file in
                Button {
                    mentionHighlight = index
                    acceptHighlightedMention()
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "doc.text")
                            .font(.system(size: CGFloat(fontSize - 3)))
                            .foregroundStyle(themeStore.accentColor())
                        VStack(alignment: .leading, spacing: 0) {
                            Text(file.title)
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 2), weight: .medium))
                                .foregroundStyle(themeStore.fontColor())
                                .lineLimit(1)
                            // The full path, not `subtitle`: two files with
                            // the same name is the normal case, and only the
                            // path tells them apart. Truncated at the HEAD so
                            // the folder nearest the file stays readable.
                            Text(PathDisplay.abbreviated(file.path))
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 4), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                                .lineLimit(1)
                                .truncationMode(.head)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    // The shared pill and zoom, so Tab here moves exactly as it
                    // does in the results and session lists.
                    .selectionPill(
                        isSelected: index == mentionHighlight,
                        themeStore: themeStore,
                        namespace: mentionSelectionNamespace,
                        geometryID: Self.mentionPillID)
                }
                .buttonStyle(.plain)
            }

            Spacer(minLength: 0)

            Text("Tab to pick · Enter to attach · Esc to dismiss")
                .font(themeStore.uiFont(size: CGFloat(fontSize - 4), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor().opacity(0.8))
                .padding(.horizontal, 8)
                .padding(.top, 2)
        }
    }

    /// The highlighted file's contents. Deliberately follows the HIGHLIGHT and
    /// not the top match: with nothing highlighted Enter still sends the
    /// message, and previewing a file the keyboard is not pointing at would
    /// suggest otherwise.
    @ViewBuilder
    private var mentionPreviewColumn: some View {
        if mentionHighlight >= 0, mentionHighlight < mentionMatches.count {
            let file = mentionMatches[mentionHighlight]
            VStack(alignment: .leading, spacing: 4) {
                Text(PathDisplay.directory(of: file.path))
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 4), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .lineLimit(1)
                    .truncationMode(.head)
                FilePreview(path: file.path)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
        } else {
            VStack {
                Spacer(minLength: 0)
                Text("Tab to preview")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor().opacity(0.7))
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity)
        }
    }
}
