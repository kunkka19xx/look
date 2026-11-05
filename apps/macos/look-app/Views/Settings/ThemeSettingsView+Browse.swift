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
            LookbookInstallSheet(entry: entry) { installed.insert(entry.id) }
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

    /// The entry's name, which opens its README.
    ///
    /// The title rather than a button beside it: "what does this actually do"
    /// is the question the name already stands for, and the row already ends
    /// in the verb that matters. Styled accent and underlined, the way
    /// `MarkdownView` styles every other link, because a title that looks like
    /// a title is one. An entry with no README stays plain text rather than
    /// offering a click that does nothing.
    @ViewBuilder
    private func name(_ entry: LookbookCatalog.Entry) -> some View {
        if let readme = catalog.readme(for: entry) {
            Button { NSWorkspace.shared.open(readme) } label: {
                Text(entry.name)
                    .underline()
                    .foregroundStyle(themeStore.accentColor())
            }
            .buttonStyle(.plain)
            .pointingHandCursor()
            .help("Open this entry's README on the web")
        } else {
            Text(entry.name)
                .foregroundStyle(themeStore.fontColor())
        }
    }

    @ViewBuilder
    private func card(_ entry: LookbookCatalog.Entry) -> some View {
        let elsewhere = !entry.platforms.contains(LookbookCatalog.thisPlatform)
        let absent = entry.requires.filter { catalog.missing.contains($0) }
        let here = installed.contains(entry.id)
            || (!entry.files.isEmpty && entry.files.allSatisfy(present.contains))

        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                name(entry)
                Text(entry.id)
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 3), design: .monospaced))
                    .foregroundStyle(themeStore.mutedTextColor())
                Spacer(minLength: 8)
                if elsewhere {
                    // Asked before anything else: an entry that cannot run
                    // here has no button, whatever is already on disk.
                    Text(entry.platforms.joined(separator: ", "))
                        .foregroundStyle(themeStore.mutedTextColor())
                } else if here {
                    // Still openable: this is how you get back to the form
                    // after deciding the folder you picked was the wrong one.
                    Button("Reconfigure") { opening = entry }
                        .buttonStyle(.plain)
                        .foregroundStyle(themeStore.accentColor())
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
    let onInstalled: () -> Void

    /// Files this would write over, asked of the engine when the sheet opens.
    /// One of them may be the copy somebody edited to point at their own
    /// folder, and a notice after the write cannot give that back. Asked
    /// rather than inferred because helper scripts live in `~/.look/bin`,
    /// which no screen lists.
    @State private var replaces: [String] = []

    @State private var files: LookbookInstall?
    @State private var previews: [String: LookbookPreview] = [:]
    @State private var failure: String?
    @State private var working = true
    /// True while a write is in flight. Each press starts an engine write,
    /// and two with different answers could finish out of order, leaving the
    /// older on disk while the sheet reported the newer as installed.
    @State private var writing = false
    /// What the user has typed or picked, keyed by file then field index.
    /// Empty means "as the author wrote it", which is what the engine reads a
    /// null as, so an untouched form installs the example unchanged.
    @State private var answers: [String: [Int: String]] = [:]

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
                    .disabled(files == nil || writing)
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

            // What the example asks for, above the commands rather than
            // below: these are the lines you would otherwise have installed
            // it and gone looking for.
            let fields = previews[file.name]?.fields ?? []
            if !fields.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(fields.enumerated()), id: \.offset) { index, field in
                        control(file.name, index, field)
                    }
                }
                .padding(.vertical, 4)
            }

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

    /// One field: a picker for a path, a popup for a choice, a box otherwise.
    @ViewBuilder
    private func control(_ file: String, _ index: Int, _ field: LookbookField) -> some View {
        let value = answers[file]?[index] ?? field.current ?? ""
        VStack(alignment: .leading, spacing: 2) {
            Text(field.label)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .medium))
                .foregroundStyle(themeStore.fontColor())

            HStack(spacing: 8) {
                switch field.kind {
                case "choice":
                    Picker("", selection: binding(file, index, value)) {
                        ForEach(field.options, id: \.self) { Text($0).tag($0) }
                    }
                    .labelsHidden()
                    .frame(maxWidth: 220)
                default:
                    TextField("", text: binding(file, index, value))
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                }

                if field.kind == "folder" || field.kind == "file" {
                    Button("Choose\u{2026}") { choose(file, index, folders: field.kind == "folder") }
                        .buttonStyle(.plain)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium))
                        .foregroundStyle(themeStore.accentColor())
                }
            }

            if let note = field.note {
                Text(note)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func binding(_ file: String, _ index: Int, _ value: String) -> Binding<String> {
        Binding(
            get: { value },
            set: {
                answers[file, default: [:]][index] = $0
                // The lines underneath describe the example as written, and
                // the point of the form is to stop them being true. Re-read
                // what Install would actually write.
                recheck(file)
            }
        )
    }

    /// Previews one declaration again with the answers filled in, so the
    /// readiness lines match the file that would land rather than the one
    /// the author shipped.
    private func recheck(_ name: String) {
        guard let files, let file = files.declarations.first(where: { $0.name == name }) else {
            return
        }
        let filled = LookbookInstall.NamedFile(
            name: file.name, contents: file.contents, answers: answers(for: file)
        )
        Task {
            let found = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.lookbookPreviewFilled(filled)
            }.value
            if let found { previews[name] = found }
        }
    }

    /// The reason a folder field exists at all: picking beats retyping
    /// somebody else's path from memory.
    private func choose(_ file: String, _ index: Int, folders: Bool) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = folders
        panel.canChooseFiles = !folders
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        answers[file, default: [:]][index] = url.path
    }

    /// Answers for one file, as the engine wants them: parallel to the
    /// declared fields, with null for anything left alone.
    private func answers(for file: LookbookInstall.NamedFile) -> [String?] {
        let fields = previews[file.name]?.fields ?? []
        guard !fields.isEmpty else { return [] }
        let given = answers[file.name] ?? [:]
        return fields.indices.map { index in
            guard let typed = given[index], typed != fields[index].current else { return nil }
            return typed
        }
    }

    private func read() async {
        let asking = LookbookPresent.Request(declarations: entry.files, scripts: entry.bin)
        replaces = await Task.detached(priority: .userInitiated) {
            EngineBridge.shared.lookbookPresent(asking)
        }.value?.all.sorted() ?? []

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
        guard let files, !writing else { return }
        writing = true
        let filled = LookbookInstall(
            declarations: files.declarations.map {
                LookbookInstall.NamedFile(
                    name: $0.name, contents: $0.contents, answers: answers(for: $0)
                )
            },
            scripts: files.scripts
        )
        Task {
            defer { writing = false }
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.lookbookInstall(filled)
            }.value
            guard let outcome else {
                failure = "Could not install it."
                return
            }

            // A reload, not the notice that one happened: the files are on
            // disk and nothing has read them yet. This is the same work
            // Cmd+Shift+; does, so an installed source brings its rows, its
            // icons and any connector it declares without the user being told
            // to go and press anything. The reload posts `lookConfigReloaded`
            // when it finishes, which is what refreshes the list behind this
            // sheet.
            //
            // Asked whenever anything landed, including a failed install. The
            // installer is partial on purpose, so a refused script leaves the
            // declaration written, and skipping the reload there would leave
            // a file on disk that no screen knows about.
            if !outcome.written.isEmpty {
                NotificationCenter.default.post(name: .lookReloadConfigRequested, object: nil)
            }

            guard outcome.problems.isEmpty else {
                failure = outcome.problems.joined(separator: "\n")
                return
            }
            onInstalled()
            dismiss()
        }
    }
}
