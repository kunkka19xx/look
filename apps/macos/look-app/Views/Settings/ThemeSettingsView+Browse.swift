import AppKit
import SwiftUI

/// Ready-made sources from lookbook, listed where what you already have is
/// listed. Browse, install, and then read what to change, all on one screen.
///
/// Nothing is fetched until this appears: the catalog is a deliberate network
/// call, not something Look does on launch.
struct LookbookBrowseSection: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @ObservedObject private var catalog = LookbookCatalog.shared

    /// File names already in the sources directory, from the list above this
    /// section. What makes "installed" outlive the visit, and what lets the
    /// sheet warn before it replaces something.
    let present: Set<String>

    @State private var filter = ""
    /// The entry whose declaration is being read before installing it.
    @State private var opening: LookbookCatalog.Entry?
    /// Ids installed during this visit, so a card can say so without the whole
    /// list being re-read.
    /// Installed during this visit. Folded in with `present` so a card does
    /// not flip back to "Install" before the list behind it has reloaded.
    @State private var installed: Set<String> = []

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Text("Browse lookbook")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                Text("ready-made sources to copy in")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                Spacer(minLength: 8)
                if case .ready = catalog.reading {
                    TextField("Filter", text: $filter)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 160)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                }
            }

            switch catalog.reading {
            case .idle, .loading:
                Text("Reading the catalog…")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            case .failed(let message):
                HStack(spacing: 8) {
                    Text(message)
                        .foregroundStyle(themeStore.warningColor())
                    Button("Try again") { catalog.load(force: true) }
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.accentColor())
                }
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
            case .ready(let entries):
                let shown = matching(entries)
                if shown.isEmpty {
                    Text("Nothing matches \u{201C}\(filter)\u{201D}.")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                } else {
                    ForEach(shown) { entry in card(entry) }
                }
            }
        }
        .task { catalog.load() }
        .sheet(item: $opening) { entry in
            LookbookInstallSheet(
                entry: entry,
                replaces: entry.files.filter(present.contains).sorted()
            ) { installed.insert(entry.id) }
                .environmentObject(themeStore)
        }
    }

    /// Filtered in memory. The catalog is already here, and going back to the
    /// engine per keystroke would stat the filesystem for every card.
    private func matching(_ entries: [LookbookCatalog.Entry]) -> [LookbookCatalog.Entry] {
        let needle = filter.trimmingCharacters(in: .whitespaces).lowercased()
        guard !needle.isEmpty else { return entries }
        return entries.filter {
            $0.name.lowercased().contains(needle)
                || $0.id.contains(needle)
                || $0.description.lowercased().contains(needle)
        }
    }

    @ViewBuilder
    private func card(_ entry: LookbookCatalog.Entry) -> some View {
        let elsewhere = !entry.platforms.contains(LookbookCatalog.thisPlatform)
        let absent = entry.requires.filter { catalog.missing.contains($0) }
        let here = installed.contains(entry.id) || entry.files.allSatisfy(present.contains)

        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Text(entry.name)
                    .foregroundStyle(themeStore.fontColor())
                Text(entry.id)
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 3), design: .monospaced))
                    .foregroundStyle(themeStore.mutedTextColor())
                Spacer(minLength: 8)
                if here {
                    // Still openable: this is how you get back to the form
                    // after deciding the folder you picked was the wrong one.
                    Button("Reconfigure") { opening = entry }
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.accentColor())
                } else if elsewhere {
                    Text(entry.platforms.joined(separator: ", "))
                        .foregroundStyle(themeStore.mutedTextColor())
                } else {
                    Button("Install") { opening = entry }
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.accentColor())
                }
            }
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))

            Text(entry.description)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
                .fixedSize(horizontal: false, vertical: true)

            // Said before installing rather than after, which is the whole
            // point of a catalog declaring what its helper scripts run.
            if !absent.isEmpty, !elsewhere {
                Text("needs \(absent.joined(separator: ", ")), which you do not have")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                    .foregroundStyle(themeStore.warningColor())
            }
        }
        .padding(10)
        .background(
            themeStore.liftColor(opacity: 0.05),
            in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
        )
    }
}

/// The declaration itself, before it is written anywhere.
///
/// lookbook's own installer says "open the .toml first: its paths and apps are
/// the author's", and that is the property a one-click install would quietly
/// drop. These are shell commands somebody else wrote, so the sheet shows them
/// and the button underneath is the second click.
struct LookbookInstallSheet: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @Environment(\.dismiss) private var dismiss

    let entry: LookbookCatalog.Entry
    /// Files already in the sources directory that this would write over.
    /// One of them may be the copy somebody edited to point at their own
    /// folder, and a notice after the write cannot give that back.
    let replaces: [String]
    let onInstalled: () -> Void

    @State private var files: LookbookInstall?
    @State private var previews: [String: LookbookPreview] = [:]
    @State private var failure: String?
    @State private var working = true

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(entry.name)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 1), weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
            Text(entry.description)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
                .fixedSize(horizontal: false, vertical: true)

            if working {
                Text("Reading it…")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            } else if let failure {
                Text(failure)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.warningColor())
            } else if let files {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(files.declarations, id: \.name) { file in
                            declaration(file)
                        }
                        if !files.scripts.isEmpty {
                            Text(
                                "Also installs \(files.scripts.map(\.name).joined(separator: ", ")) "
                                    + "into ~/.look/bin."
                            )
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                            .foregroundStyle(themeStore.mutedTextColor())
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

            if !replaces.isEmpty {
                Text(
                    "Replaces \(replaces.joined(separator: ", ")), overwriting any changes "
                        + "you made to it."
                )
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                .foregroundStyle(themeStore.warningColor())
                .fixedSize(horizontal: false, vertical: true)
            }

            HStack(spacing: 8) {
                Spacer()
                Button("Cancel") { dismiss() }
                // Named for what it does: the second press is the one that
                // takes somebody's edits away, so it should not say Install.
                Button(replaces.isEmpty ? "Install" : "Replace") { install() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(files == nil)
            }
        }
        .padding(16)
        .frame(width: 620, height: 520)
        .task { await read() }
    }

    @ViewBuilder
    private func declaration(_ file: LookbookInstall.NamedFile) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(file.name)
                .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                .foregroundStyle(themeStore.secondaryTextColor())

            // What it would want changed, said here rather than after
            // installing. The same check the Extend screen runs, so this
            // cannot promise something the screen then contradicts.
            ForEach(previews[file.name]?.blocks ?? []) { block in
                ForEach(block.needs) { need in
                    Text("\(block.id): \(need.message)")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                        .foregroundStyle(themeStore.warningColor())
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            Text(file.contents)
                .font(.system(size: CGFloat(themeStore.settings.fontSize - 3), design: .monospaced))
                .foregroundStyle(themeStore.fontColor())
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(
                    themeStore.liftColor(opacity: 0.05),
                    in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
                )
        }
    }

    private func read() async {
        guard let fetched = await LookbookCatalog.shared.fetchFiles(for: entry) else {
            failure = "Could not read \(entry.id) from lookbook."
            working = false
            return
        }
        var read: [String: LookbookPreview] = [:]
        for file in fetched.declarations {
            let contents = file.contents
            // Nil means it is not a file of blocks at all. A preview that
            // came back with problems is still installable: part of it
            // parsed, and the sheet shows what did not.
            guard let preview = await Task.detached(priority: .userInitiated, operation: {
                EngineBridge.shared.lookbookPreview(toml: contents)
            }).value else {
                failure = "\(file.name) is not a declaration Look can read."
                working = false
                return
            }
            read[file.name] = preview
        }
        files = fetched
        previews = read
        working = false
    }

    private func install() {
        guard let files else { return }
        Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.lookbookInstall(files)
            }.value
            guard let outcome, outcome.problems.isEmpty else {
                failure = outcome?.problems.joined(separator: "\n") ?? "Could not install it."
                return
            }
            onInstalled()
            // A reload, not the notice that one happened: the file is on disk
            // and nothing has read it yet. This is the same work Cmd+Shift+;
            // does, so an installed source brings its rows, its icons and any
            // connector it declares without the user being told to go and
            // press anything. The reload posts `lookConfigReloaded` when it
            // finishes, which is what refreshes the list behind this sheet.
            NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
            dismiss()
        }
    }
}
