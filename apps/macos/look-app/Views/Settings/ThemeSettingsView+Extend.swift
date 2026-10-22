import AppKit
import SwiftUI

extension ThemeSettingsView {
    /// What Look has been extended with: the blocks you wrote, and the ones an
    /// app installed. Read-only for now; it answers "where did this row come
    /// from" and "is that connector actually running".
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

        if inventory.problems.isEmpty {
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
            HStack(spacing: 8) {
                Text(fileName(declaration.file))
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                    .foregroundStyle(themeStore.secondaryTextColor())
                Spacer(minLength: 8)
                link("Edit") { edit(declaration.file) }
                link("Reveal") {
                    NSWorkspace.shared.activateFileViewerSelecting([
                        URL(fileURLWithPath: declaration.file)
                    ])
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
            }
        }
        .padding(10)
        .background(
            themeStore.liftColor(opacity: 0.05),
            in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
        )
    }

    private func link(_ title: String, _ act: @escaping () -> Void) -> some View {
        Button(title, action: act)
            .buttonStyle(.plain)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium))
            .foregroundStyle(themeStore.accentColor())
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
