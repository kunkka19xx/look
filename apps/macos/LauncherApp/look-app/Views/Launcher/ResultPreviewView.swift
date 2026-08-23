import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ResultPreviewView: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let result: LauncherResult
    /// The levels this row was reached through, for a `preview` that names
    /// `{parent.*}`. Empty for every row that is not inside a drill-down.
    var rowAncestorsJSON: String = "[]"
    /// Quick Actions for this result, rendered beneath the header (info + actions
    /// panel). Empty for results with no actions.
    var quickActions: [QuickActionDescriptor] = []
    var quickActionStates: [String: ActionState] = [:]
    var quickActionInfo: [String: [String: InfoValue]] = [:]
    var pendingQuickActionItems: Set<String> = []
    /// Actions with something applying: their controls render inert (see
    /// `LauncherView.busyQuickActionIds`).
    var busyQuickActionIds: Set<String> = []
    /// Changes each time the launcher opens, replaying the quick-action cascade.
    var quickActionsRevealToken: UInt64 = 0
    var onRunQuickAction: (QuickActionDescriptor, ActionIntent) -> Void = { _, _ in }
    var onActivateQuickActionItem: (QuickActionDescriptor, QuickActionListItem) -> Void = { _, _ in }
    var onDeleteClipboard: (() -> Void)? = nil
    /// Process-finder preview inputs (only set for `.process` results).
    var processDetail: ProcessDetail? = nil
    var processCPU: Double? = nil
    var isMeasuringProcessCPU: Bool = false
    /// The Cmd+K action menu, floated under the header rather than laid out, so
    /// a row's verbs cost the preview nothing until they are asked for.
    var isActionMenuOpen: Bool = false
    var actionMenuIndex: Int = 0
    /// What the menu lists. Not always `quickActions`: with an empty query the
    /// launchpad's own controls take its place.
    var actionMenuDescriptors: [QuickActionDescriptor] = []
    /// Activating a row of the Cmd+K menu, which is not the same as running it:
    /// a target with a `confirm` asks first.
    var onActivateActionMenuRow: (QuickActionDescriptor) -> Void = { _ in }

    @State private var folderListing: FolderListing?
    @State private var trashItemCount: Int?
    /// Steps of the declared block behind an `.action` row, read on selection,
    /// with the file that declared it.
    @State private var blockSteps: [String] = []
    @State private var blockFile: String?
    /// Output of the block's declared `preview`, for rows that have one.
    @State private var blockPreview: SourcePreview?

    /// The menu for a preview that has no content area to pin it into. Padded
    /// clear of the header so it still reads as attached to the row above it.
    @ViewBuilder
    private var floatingActionMenu: some View {
        if isActionMenuOpen {
            actionMenu
                .padding(.horizontal, 16)
                .padding(.top, 84)
        }
    }

    /// The Cmd+K popup, pinned to the top of the content area so it opens flush
    /// under the header and floats over whatever the preview is showing.
    private var actionMenu: some View {
        ActionMenuView(
            descriptors: actionMenuDescriptors,
            states: quickActionStates,
            focusedIndex: actionMenuIndex,
            themeStore: themeStore,
            onActivate: onActivateActionMenuRow
        )
        .transition(.opacity.combined(with: .move(edge: .top)))
        .zIndex(1)
    }

    /// Actions with live details worth reading (Bluetooth's paired devices).
    /// Their verbs live in the Cmd+K menu; only what they know stays here.
    private var infoOnlyQuickActions: [QuickActionDescriptor] {
        quickActions.filter { !$0.info.isEmpty }
    }

    /// A System Settings pane result (its "path" is a URL scheme, not a file).
    private var isSetting: Bool {
        result.id.hasPrefix("setting:")
    }

    /// The pinned Trash quick folder is TCC-protected, so it can't be listed
    /// like a normal folder - it gets a Finder-backed summary instead.
    private var isTrash: Bool {
        result.kind == .folder
            && DeleteTargetLogic.isTrashPath(result.path, homeDirectory: NSHomeDirectory())
    }

    private func folderCountText(_ listing: FolderListing) -> String? {
        var parts: [String] = []
        if listing.folderCount > 0 {
            parts.append("\(listing.folderCount) folder\(listing.folderCount == 1 ? "" : "s")")
        }
        if listing.fileCount > 0 {
            parts.append("\(listing.fileCount) file\(listing.fileCount == 1 ? "" : "s")")
        }
        return parts.isEmpty ? nil : parts.joined(separator: ", ")
    }

    private static let modifiedDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()
    private static let clipboardDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .medium
        return formatter
    }()

    private var clipboardIcon: NSImage {
        NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: nil)
            ?? NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
            ?? NSWorkspace.shared.icon(for: .plainText)
    }

    /// The synthesized calculator row - no file/bundle behind it, so it gets
    /// its own branch like clipboard rows do.
    private var isCalcResult: Bool {
        if case .calc = SyntheticRow.classify(resultID: result.id) { return true }
        return false
    }

    private var calcIcon: NSImage { LauncherCalcFeature.icon() }

    /// The planner-proposed action row: no file behind it, so it gets its own
    /// hero panel - icon, the plan (the point of the row), a type badge, and
    /// the key hints - all styled from `AIActionAppearance` so new tools reuse
    /// this panel unchanged.
    private var aiActionToolID: String? {
        if case .aiAction(let toolID) = SyntheticRow.classify(resultID: result.id) {
            return toolID
        }
        return nil
    }

    private func aiActionPreview(_ toolID: String) -> some View {
        let look = AIActionAppearance.look(forToolID: toolID)
        return VStack(spacing: 14) {
            Spacer(minLength: 0)

            Image(nsImage: AIActionAppearance.icon(forToolID: toolID))
                .resizable()
                .scaledToFit()
                .frame(width: 52, height: 52)
                .foregroundStyle(themeStore.accentColor())

            Text(result.title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 5), weight: .bold))
                .foregroundStyle(themeStore.fontColor())
                .multilineTextAlignment(.center)
                .lineLimit(3)
                .minimumScaleFactor(0.6)

            KindBadge(kind: look.typeName.lowercased())

            VStack(alignment: .leading, spacing: 8) {
                hintRow(key: "↵", text: look.verb)
                hintRow(key: "⌘Z", text: "Undo after it runs")
                hintRow(key: "Esc", text: "Dismiss")
            }
            .padding(.top, 6)

            Spacer(minLength: 0)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// The synthesized rows that open a URL - a meeting to join, a way to
    /// reach someone. No file behind either, so they take the same centered
    /// hero shape as the action and calc rows.
    private var linkURL: String? {
        switch SyntheticRow.classify(resultID: result.id) {
        case .meeting(let url), .call(let url): return url
        default: return nil
        }
    }

    private func linkIcon(_ url: String) -> NSImage {
        NSImage(
            systemSymbolName: LinkRowAppearance.symbol(forURL: url), accessibilityDescription: nil)
            ?? NSWorkspace.shared.icon(for: .plainText)
    }

    /// "Teams  ·  14:30  ·  in 4 min", or "FaceTime audio  ·  mobile  ·  +1 …",
    /// dropping whichever half is missing.
    private var linkDetailLine: String? {
        let parts = [result.linkKindLabel, result.linkDetail].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: "  ·  ")
    }

    private func linkPreview(_ url: String) -> some View {
        VStack(spacing: 14) {
            Spacer(minLength: 0)

            Image(nsImage: linkIcon(url))
                .resizable()
                .scaledToFit()
                .frame(width: 52, height: 52)
                .foregroundStyle(themeStore.accentColor())

            Text(result.title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 5), weight: .bold))
                .foregroundStyle(themeStore.fontColor())
                .multilineTextAlignment(.center)
                .lineLimit(3)
                .minimumScaleFactor(0.6)

            // Not a `KindBadge`: it renders `kind.capitalized`, which would turn
            // "GoToMeeting" into "Gotomeeting". Provider names are the one label
            // here whose casing is the brand.
            if let detail = linkDetailLine {
                Text(detail)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .multilineTextAlignment(.center)
            }

            VStack(alignment: .leading, spacing: 8) {
                hintRow(key: "↵", text: openHint(url))
                hintRow(key: "Esc", text: "Dismiss")
            }
            .padding(.top, 6)

            // Where Enter actually goes. An invite is written by whoever sent
            // it, so naming the host is the one thing that lets a reader catch
            // a link that is not the meeting it claims to be.
            if let host = URL(string: url)?.host {
                Text(host)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer(minLength: 0)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// What Enter will actually do, in the words of the destination.
    private func openHint(_ url: String) -> String {
        let lower = url.lowercased()
        if lower.hasPrefix("sms:") || lower.hasPrefix("imessage:") { return "Open Messages" }
        if lower.hasPrefix("tel:") { return "Call through your iPhone" }
        if lower.hasPrefix("facetime") { return "Start the FaceTime call" }
        return "Join the meeting"
    }

    private func hintRow(key: String, text: String) -> some View {
        HStack(spacing: 10) {
            Text(key)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                .foregroundStyle(themeStore.accentColor())
                .frame(minWidth: 36)
                .padding(.vertical, 4)
                .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            Text(text)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                .foregroundStyle(themeStore.secondaryTextColor())
        }
        .frame(width: 230, alignment: .leading)
    }

    /// No file/bundle behind this row, so - like a web-search suggestion -
    /// it gets a centered hero layout instead of the header+detail one above:
    /// icon, the answer (the point of the row), the expression it came from,
    /// then the hint.
    private var calcPreview: some View {
        VStack(spacing: 14) {
            Spacer(minLength: 0)

            Image(nsImage: calcIcon)
                .resizable()
                .scaledToFit()
                .frame(width: 56, height: 56)

            Text(result.title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 14), weight: .bold))
                .foregroundStyle(themeStore.fontColor())
                .lineLimit(1)
                .minimumScaleFactor(0.5)

            Text(result.calcExpression ?? "")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())

            Text("Press \(AppConstants.Launcher.Calc.enterToCopyHint)")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            Spacer(minLength: 0)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var largeIcon: NSImage {
        if result.id.hasPrefix("setting:") {
            let settingsPath = "/System/Applications/System Settings.app"
            if FileManager.default.fileExists(atPath: settingsPath) {
                return NSWorkspace.shared.icon(forFile: settingsPath)
            }
            let legacyPath = "/System/Applications/System Preferences.app"
            return NSWorkspace.shared.icon(forFile: legacyPath)
        }
        return NSWorkspace.shared.icon(forFile: result.path)
    }

    private var bundleInfo: (version: String?, size: String, modified: String?) {
        var version: String? = nil
        var modified: String? = nil
        var totalSize: Int64 = 0

        if result.id.hasPrefix("setting:") || result.kind == .app {
            let appPath = result.id.hasPrefix("setting:")
                ? "/System/Applications/System Settings.app"
                : result.path

            if let bundle = Bundle(path: appPath) {
                version = bundle.infoDictionary?["CFBundleShortVersionString"] as? String
                    ?? bundle.infoDictionary?["CFBundleVersion"] as? String
            }

            if let attrs = try? FileManager.default.attributesOfItem(atPath: appPath) {
                if let modDate = attrs[.modificationDate] as? Date {
                    modified = Self.modifiedDateFormatter.string(from: modDate)
                }
                if let size = attrs[.size] as? Int64 {
                    totalSize = size
                }
            }
        } else {
            if let attrs = try? FileManager.default.attributesOfItem(atPath: result.path) {
                if let size = attrs[.size] as? Int64 {
                    totalSize = size
                }
                if let modDate = attrs[.modificationDate] as? Date {
                    modified = Self.modifiedDateFormatter.string(from: modDate)
                }
            }
        }

        let sizeStr = formatFileSize(totalSize)
        return (version, sizeStr, modified)
    }

    private func formatFileSize(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }

    var body: some View {
        if result.kind == .action {
            actionPreview.overlay(alignment: .top) { floatingActionMenu }
        } else if result.kind == .process {
            processPreview
        } else if result.kind == .clipboard {
            clipboardPreview
        } else if isCalcResult {
            calcPreview
        } else if let toolID = aiActionToolID {
            aiActionPreview(toolID)
        } else if let linkURL {
            linkPreview(linkURL)
        } else {
        let info = bundleInfo

            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    Image(nsImage: largeIcon)
                        .resizable()
                        .frame(width: 48, height: 48)
                        .shadow(color: .black.opacity(0.3), radius: 4, x: 0, y: 2)

                    VStack(alignment: .leading, spacing: 4) {
                        Text(result.title)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: .semibold))
                            .foregroundStyle(themeStore.fontColor())
                            .lineLimit(2)

                        HStack(spacing: 6) {
                            KindBadge(kind: result.kind.rawValue)
                            if result.kind == .folder {
                                if isTrash {
                                    if let trashItemCount {
                                        Text("\(trashItemCount) item\(trashItemCount == 1 ? "" : "s")")
                                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                            .foregroundStyle(themeStore.secondaryTextColor())
                                    }
                                } else if let listing = folderListing,
                                   let counts = folderCountText(listing) {
                                    Text(counts)
                                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                        .foregroundStyle(themeStore.secondaryTextColor())
                                }
                            } else {
                                Text(info.size)
                                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.secondaryTextColor())
                            }
                        }
                    }
                    Spacer()
                }

                ZStack(alignment: .topLeading) {
                    VStack(alignment: .leading, spacing: 12) {
                // Info only: an action's live details (Bluetooth's paired
                // devices) are what the panel is for. Its verbs are in Cmd+K.
                if !infoOnlyQuickActions.isEmpty {
                    QuickActionsSection(
                        descriptors: infoOnlyQuickActions,
                        states: quickActionStates,
                        info: quickActionInfo,
                        pendingItems: pendingQuickActionItems,
                        busyActionIds: busyQuickActionIds,
                        themeStore: themeStore,
                        revealToken: quickActionsRevealToken,
                        onRun: { _, _ in },
                        onActivateItem: onActivateQuickActionItem,
                        controlHidden: true
                    )
                }

                if result.kind == .file {
                    FilePreview(path: result.path)
                }

                if result.kind == .folder {
                    if isTrash {
                        TrashSummaryView(itemCount: trashItemCount, themeStore: themeStore)
                    } else {
                        FolderPreviewView(path: result.path, listing: folderListing)
                    }
                }

                if let version = info.version {
                    InfoRow(label: "Version", value: version)
                }

                InfoRow(label: "Kind", value: result.kind.rawValue.capitalized)

                // Settings panes have a URL-scheme "path" and a meaningless file
                // date; hide both for them (only the actions matter there).
                if !isSetting {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Path")
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                        Text(result.path)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                            .foregroundStyle(themeStore.secondaryTextColor())
                            .lineLimit(3)
                    }

                    if let modified = info.modified {
                        InfoRow(label: "Modified", value: modified)
                    }
                }

                if result.kind != .file && result.kind != .folder {
                    Spacer()
                }
                    }

                    if isActionMenuOpen {
                        actionMenu
                    }
                }
            }
            .padding(12)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .task(id: result.kind == .folder ? result.path : "") {
                guard result.kind == .folder else {
                    folderListing = nil
                    trashItemCount = nil
                    return
                }
                if isTrash {
                    // Don't list ~/.Trash (TCC) and don't prompt for Automation
                    // just by previewing - only show a count if already granted.
                    folderListing = nil
                    trashItemCount = EmptyTrashCommand.itemCount(promptIfNeeded: false)
                    return
                }
                folderListing = nil
                let path = result.path
                let listing = await FolderListingService.list(path: path)
                // .task(id:) cancels this closure when the result changes,
                // but the detached worker keeps running - guard against
                // stale assignment when the user moved on to another folder.
                if Task.isCancelled { return }
                folderListing = listing
            }
        }
    }

    /// A declared block has no file to describe, so the panel answers the only
    /// question that matters before Enter: exactly what is about to run.
    private var actionPreview: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(systemName: "bolt.fill")
                    .font(.system(size: 26))
                    .foregroundStyle(themeStore.accentColor())
                    .frame(width: 48, height: 48)

                VStack(alignment: .leading, spacing: 4) {
                    Text(result.title)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                        .lineLimit(2)
                    Text(result.subtitle ?? "")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                }
                Spacer()
            }

            let appIcons = SourceBlockIcons.appIcons(forSteps: blockSteps, limit: 8)
            if !appIcons.isEmpty {
                // What the steps will actually bring up. Only steps that name an
                // app contribute, so the strip never implies more than the block
                // does.
                HStack(spacing: 6) {
                    ForEach(Array(appIcons.enumerated()), id: \.offset) { _, icon in
                        Image(nsImage: icon)
                            .resizable()
                            .frame(width: 24, height: 24)
                    }
                }
            }

            if !blockSteps.isEmpty {
                Text("Enter runs")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .medium))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }

            // The steps ARE shell, so they get the same block an AI answer's
            // code gets: highlighted, selectable, and copyable in one press.
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if !blockSteps.isEmpty {
                        AICodeBlockView(
                            code: blockSteps.joined(separator: "\n"),
                            language: "sh",
                            themeStore: themeStore
                        )
                    }
                    if let preview = blockPreview {
                        blockPreviewBody(preview)
                    }
                }
            }

            Spacer(minLength: 0)

            if let file = blockFile {
                Divider().overlay(themeStore.secondaryTextColor().opacity(0.2))
                InfoRow(label: "Declared in", value: (file as NSString).abbreviatingWithTildeInPath)
                // A row that names its own path reveals that, like every other
                // row with one, so the chord belongs to the declaration only
                // when the row has nothing of its own to point at.
                if result.path.isEmpty {
                    hintRow(key: "⌘F", text: "Reveal that file")
                }
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task(id: result.id) {
            let candidateID = result.id
            let ancestors = rowAncestorsJSON
            let block = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.sourceBlock(
                    candidateID: candidateID, ancestorsJSON: ancestors)
            }.value
            // The detached read outlives a cancelled task, so a late answer must
            // not populate the panel of a row the user has already left.
            if Task.isCancelled { return }
            blockSteps = block?.steps ?? []
            blockFile = block?.file

            // The declared `preview` runs a command, so it is read separately
            // and only after the cheap details are on screen.
            blockPreview = nil
            let row = (id: result.id, title: result.title, path: result.path)
            let preview = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.sourcePreview(
                    candidateID: candidateID,
                    rowID: row.id,
                    rowTitle: row.title,
                    rowPath: row.path,
                    ancestorsJSON: ancestors
                )
            }.value
            if Task.isCancelled { return }
            blockPreview = preview
        }
    }

    /// A block's `preview` output, or the reason it could not run. A failure is
    /// shown rather than swallowed: a preview that silently does nothing reads
    /// as the feature being broken.
    @ViewBuilder
    private func blockPreviewBody(_ preview: SourcePreview) -> some View {
        if let error = preview.error {
            Text(error)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        } else if !preview.text.isEmpty {
            Text(preview.text)
                .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                .foregroundStyle(themeStore.secondaryTextColor())
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var clipboardPreview: some View {
        let content = result.clipboardContent ?? ""
        let capturedAt = result.clipboardCapturedAt.map { Self.clipboardDateFormatter.string(from: $0) } ?? "Unknown"
        let characterCount = result.clipboardCharacterCount ?? content.count
        let lineCount = result.clipboardLineCount ?? max(1, content.split(whereSeparator: \.isNewline).count)
        let previewFont = NSFont.monospacedSystemFont(
            ofSize: CGFloat(themeStore.settings.fontSize - 1),
            weight: .regular
        )

        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Image(nsImage: clipboardIcon)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 34, height: 34)
                    .foregroundStyle(themeStore.accentColor())
                VStack(alignment: .leading, spacing: 2) {
                    Text("Clipboard item")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 1), weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                    Text("Captured \(capturedAt)")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
                Spacer()

                if let onDeleteClipboard {
                    Button {
                        onDeleteClipboard()
                    } label: {
                        Label("Delete", systemImage: "trash")
                    }
                    .buttonStyle(.plain)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                    .foregroundStyle(themeStore.dangerColor().opacity(0.95))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(themeStore.dangerColor().opacity(0.16), in: Capsule())
                }
            }

            HStack(spacing: 8) {
                KindBadge(kind: "clipboard")
                Text("\(characterCount) chars")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
                Text("\(lineCount) lines")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }

            Text("Preview")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .medium))
                .foregroundStyle(themeStore.mutedTextColor())

            // TextKit, not SwiftUI Text: Text lays out the whole clip
            // synchronously and drops frames on long content.
            HighlightedTextView(
                attributed: NSAttributedString(string: content),
                font: previewFont,
                defaultColor: NSColor(themeStore.secondaryTextColor())
            )
            .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 8, style: .continuous))

            InfoRow(label: "Kind", value: "Clipboard")
            InfoRow(label: "Captured", value: capturedAt)

            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    // MARK: - Process preview

    private var processIcon: NSImage {
        result.processPID.map(LauncherProcessFeature.icon) ?? NSWorkspace.shared.icon(for: .unixExecutable)
    }

    private func formattedStart(_ epoch: UInt64) -> String {
        Self.modifiedDateFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(epoch)))
    }

    private var cpuValueText: String {
        if let processCPU {
            return String(format: "%.1f%%", processCPU)
        }
        return isMeasuringProcessCPU ? "Measuring…" : "Press Enter to measure"
    }

    private var portsText: String {
        let ports = result.processPorts ?? []
        return ports.isEmpty ? "None" : ports.map { ":\($0)" }.joined(separator: "  ")
    }

    private var processPreview: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Image(nsImage: processIcon)
                    .resizable()
                    .frame(width: 40, height: 40)

                VStack(alignment: .leading, spacing: 4) {
                    Text(result.title)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                        .lineLimit(2)
                    HStack(spacing: 6) {
                        KindBadge(kind: "process")
                        if let pid = result.processPID {
                            Text("PID \(pid)")
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.secondaryTextColor())
                        }
                    }
                }
                Spacer()
            }

            // Command line (argv).
            if let detail = processDetail, !detail.cmdline.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Command")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                    Text(detail.cmdline)
                        .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                        .foregroundStyle(themeStore.secondaryTextColor())
                        .lineLimit(4)
                        .textSelection(.enabled)
                }
            }

            if let detail = processDetail {
                InfoRow(label: "Memory", value: formatFileSize(Int64(detail.memoryKB) * 1024))
                if !detail.user.isEmpty {
                    InfoRow(label: "User", value: detail.user)
                }
                InfoRow(label: "Parent PID", value: String(detail.ppid))
                if let start = detail.startEpoch {
                    InfoRow(label: "Started", value: formattedStart(start))
                }
            }

            InfoRow(label: "CPU", value: cpuValueText)
            InfoRow(label: "Ports", value: portsText)

            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

struct KindBadge: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let kind: String

    private var color: Color {
        switch kind {
        case "app": return themeStore.accentColor()
        case "file": return themeStore.successColor()
        case "folder": return themeStore.warningColor()
        case "clipboard": return themeStore.accentColor()
        default: return themeStore.mutedTextColor()
        }
    }

    private var foreground: Color {
        switch kind {
        case "file":
            return themeStore.onSuccessColor()
        case "folder":
            return themeStore.onWarningColor()
        default:
            return themeStore.onAccentColor()
        }
    }

    var body: some View {
        Text(kind.capitalized)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium))
            .foregroundStyle(foreground)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.8), in: Capsule())
    }
}

struct InfoRow: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
            Spacer()
            Text(value)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
        }
    }
}
