import AppKit
import SwiftUI

extension ThemeSettingsView {
    /// What Look has been extended with: the blocks you wrote, and the ones an
    /// app installed. It answers "where did this row come from" and "is that
    /// connector actually running", and is where one goes off or away.
    var extendTab: some View {
        ExtendSettingsTab()
    }
}

struct ExtendSettingsTab: View {
    @EnvironmentObject private var themeStore: ThemeStore

    /// The three states this screen has. An unread inventory must not render
    /// as an empty one: every message below is a claim about what is installed,
    /// and this is the screen someone opens to check exactly that.
    private enum Reading {
        case pending
        case unreadable
        case ready(Inventory)
    }

    @State private var reading: Reading = .pending
    /// Bumped by a reload, which cancels the read in flight and starts a fresh
    /// one. That is also what stops an older answer from landing last.
    @State private var reloads = 0
    /// Why the last switch or removal did nothing. Shown rather than swallowed:
    /// this screen's whole job is saying what is installed and what is not.
    @State private var failure: String?

    private enum Metrics {
        static let sectionSpacing: CGFloat = 14
        static let rowSpacing: CGFloat = 8
    }

    /// One file, with the blocks it declared. The file is the unit a user
    /// installs and deletes, so it is the unit this lists.
    private struct Declaration: Identifiable {
        let file: String
        let blocks: [InstalledBlock]
        let problems: [Reported]
        var id: String { file }
        /// A file that declared nothing usable has no block to ask, so its
        /// directory answers instead: that is the whole case this screen is
        /// for, since such a file is invisible everywhere else.
        var fromApp: Bool {
            blocks.first?.fromApp ?? file.contains("/.look/connectors/")
        }
        /// A property of the file, since the file is what the switch writes
        /// to. Blocks that disagree read as on, so one press turns the
        /// remainder off rather than reporting a state no row shows.
        var off: Bool {
            !blocks.isEmpty && blocks.allSatisfy { !$0.enabled }
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Metrics.sectionSpacing) {
                switch reading {
                case .pending:
                    EmptyView()
                case .unreadable:
                    Text("Could not read what is installed.")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.warningColor())
                case .ready(let inventory):
                    sections(of: inventory)
                }
                if let failure {
                    Text(failure)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.warningColor())
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .task(id: reloads) { await read() }
        // Reloading is how an edit made from here takes effect, so the list
        // has to follow it. Otherwise Edit, save, reload leaves the problems
        // you just fixed still on screen.
        .onReceive(NotificationCenter.default.publisher(for: .lookConfigReloaded)) { _ in
            reloads += 1
        }
    }

    @MainActor
    private func read() async {
        let found = await Task.detached { EngineBridge.shared.installedInventory() }.value
        // A reload cancelled this read and started a newer one, whose answer
        // is the one that should land.
        guard !Task.isCancelled else { return }
        reading = found.map(Reading.ready) ?? .unreadable
    }

    @ViewBuilder
    private func sections(of inventory: Inventory) -> some View {
        let declarations = declarations(of: inventory)
        section(
            "Installed by apps",
            "~/.look/connectors",
            declarations.filter(\.fromApp),
            empty: "No app has installed anything yet."
        )
        section(
            "Yours",
            "~/.look/sources",
            declarations.filter { !$0.fromApp },
            empty: "Nothing declared yet."
        )

        // The installed list is right above, so the browser can tell which
        // entries are already here rather than only remembering this visit.
        LookbookBrowseSection(
            // Only what the user installed. A connector manifest an app put
            // in ~/.look/connectors can share a name with a catalog entry,
            // and installing would not touch it.
            present: Set(inventory.blocks.filter { !$0.fromApp }.map { fileName($0.file) })
        )

        if inventory.problems.isEmpty, inventory.blocks.allSatisfy(\.needs.isEmpty) {
            Text("No problems.")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        }
    }

    private func declarations(of inventory: Inventory) -> [Declaration] {
        var order: [String] = []
        var grouped: [String: [InstalledBlock]] = [:]
        var reported: [String: [Reported]] = [:]
        for block in inventory.blocks {
            if grouped[block.file] == nil { order.append(block.file) }
            grouped[block.file, default: []].append(block)
        }
        // A file that failed to parse produced no blocks, so it would be
        // missing from a list built from blocks alone. That file is exactly
        // the one worth showing.
        for problem in inventory.problems {
            if grouped[problem.file] == nil, reported[problem.file] == nil {
                order.append(problem.file)
            }
            reported[problem.file, default: []].append(problem)
        }
        return order.map {
            Declaration(file: $0, blocks: grouped[$0] ?? [], problems: reported[$0] ?? [])
        }
    }

    @ViewBuilder
    private func section(
        _ title: String, _ where: String, _ items: [Declaration], empty: String
    ) -> some View {
        VStack(alignment: .leading, spacing: Metrics.rowSpacing) {
            HStack(spacing: 6) {
                Text(title)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                Text(`where`)
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 3), design: .monospaced))
                    .foregroundStyle(themeStore.mutedTextColor())
            }

            if items.isEmpty {
                Text(empty)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            } else {
                ForEach(items) { declaration in
                    declarationRow(declaration)
                }
            }
        }
    }

    private func declarationRow(_ declaration: Declaration) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Text(fileName(declaration.file))
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                    .foregroundStyle(themeStore.secondaryTextColor())
                Spacer(minLength: 8)
                // Nothing parsed means no block to carry the key, so there is
                // nothing to turn off. Removing it is still the way out.
                if !declaration.blocks.isEmpty {
                    pill(declaration.off ? "Turn on" : "Turn off") {
                        setEnabled(declaration, declaration.off)
                    }
                }
                pill("Edit") { edit(declaration.file) }
                pill("Reveal") {
                    NSWorkspace.shared.activateFileViewerSelecting([
                        URL(fileURLWithPath: declaration.file)
                    ])
                }
                // Only what the user wrote. Deleting a manifest an app
                // installed invites that app to write it back on its next
                // launch, so there the honest verb is off.
                if !declaration.fromApp {
                    pill("Remove", themeStore.warningColor()) { remove(declaration) }
                }
            }

            ForEach(declaration.problems) { problem in
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: CGFloat(themeStore.settings.fontSize - 3)))
                    Text(problem.message)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 0)
                }
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.warningColor())
            }

            ForEach(declaration.blocks) { block in
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 8) {
                        Text(block.name)
                            .foregroundStyle(block.enabled ? themeStore.fontColor() : themeStore.mutedTextColor())
                        Text(block.kind)
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 3), design: .monospaced))
                            .foregroundStyle(themeStore.mutedTextColor())
                        Spacer(minLength: 8)
                        Text(state(of: block))
                            .foregroundStyle(themeStore.mutedTextColor())
                    }
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))

                    // Indented under the block, because this is a line to go
                    // and change in that block rather than something wrong
                    // with the file as a whole.
                    ForEach(block.needs) { need in
                        Text(need.message)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                            .foregroundStyle(themeStore.warningColor())
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.leading, 12)
                    }
                }
            }
        }
        .padding(10)
        .background(
            themeStore.liftColor(opacity: 0.05),
            in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
        )
    }

    /// One action, in a pill of its own.
    ///
    /// Bordered rather than bare words: four verbs in a row read as one phrase
    /// without an edge between them, and one of them moves a file to the Trash.
    /// The neutral fill and border every other pill in Settings uses, so `tint`
    /// is left to say which verb is the odd one.
    private func pill(
        _ title: String, _ tint: Color? = nil, _ act: @escaping () -> Void
    ) -> some View {
        Button(action: act) {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium))
                .foregroundStyle(tint ?? themeStore.accentColor())
                .padding(.horizontal, 9)
                .padding(.vertical, 3)
                .background(themeStore.liftColor(opacity: 0.16), in: Capsule())
                .overlay(Capsule().strokeBorder(themeStore.liftColor(opacity: 0.22), lineWidth: 1))
        }
        .buttonStyle(.plain)
        .contentShape(Capsule())
    }

    /// Writes `enabled` into every block of the file and reloads.
    ///
    /// A reload rather than the notice that one happened: the bytes are on disk
    /// and nothing has read them yet. The same work an edit by hand needs, so a
    /// connector just turned off is stopped by the one path that starts them,
    /// and the list refreshes when it finishes.
    private func setEnabled(_ declaration: Declaration, _ on: Bool) {
        failure = nil
        let file = declaration.file
        Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.setSourceEnabled(file: file, on)
            }.value
            guard let outcome, outcome.problems.isEmpty else {
                failure = outcome?.problems.joined(separator: "\n")
                    ?? "Could not change \(fileName(file))."
                return
            }
            if outcome.changed {
                NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
            }
        }
    }

    /// To the Trash, not unlinked. Unconfirmed for the same reason Cmd+D is:
    /// the Trash is the undo, and a declaration put back is a declaration
    /// again.
    private func remove(_ declaration: Declaration) {
        failure = nil
        let file = declaration.file
        NSWorkspace.shared.recycle([URL(fileURLWithPath: file)]) { _, error in
            Task { @MainActor in
                if let error {
                    failure = "Could not remove \(fileName(file)): \(error.localizedDescription)"
                    return
                }
                NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
            }
        }
    }

    /// Through the declared `text_editor`, so this opens whatever `Cmd+E` on a
    /// row would. Undeclared means the system handler, which is what every
    /// other undeclared tool means.
    private func edit(_ file: String) {
        let row = ToolActionRow(candidateID: "", title: fileName(file), path: file)
        let action = AppConstants.Launcher.Tools.editAction
        Task.detached(priority: .userInitiated) {
            let outcome = EngineBridge.shared.performToolAction(
                action, row: row, isDirectory: false
            )
            let url = URL(fileURLWithPath: outcome?.path ?? file)
            switch outcome?.kind {
            case .performed, .failed, .none:
                break
            case .application:
                if let tool = outcome?.tool {
                    await MainActor.run { openWith(tool: tool, url: url) }
                }
            case .shell, .systemDefault, .unavailable:
                await MainActor.run { NSWorkspace.shared.open(url) }
            }
        }
    }

    @MainActor
    private func openWith(tool: String, url: URL) {
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: tool)
            ?? NSWorkspace.shared.urlForApplication(toOpen: url)
        else { return }
        NSWorkspace.shared.open(
            [url], withApplicationAt: app, configuration: NSWorkspace.OpenConfiguration()
        )
    }

    /// Only a `connect` block has a process to be up or down. Everything else
    /// is a declaration, and saying "stopped" about a folder listing would be
    /// inventing a state it does not have.
    private func state(of block: InstalledBlock) -> String {
        if !block.enabled { return "off" }
        guard block.kind == "connect" else { return "" }
        return block.running ? "running" : "not running"
    }

    private func fileName(_ path: String) -> String {
        (path as NSString).lastPathComponent
    }
}
