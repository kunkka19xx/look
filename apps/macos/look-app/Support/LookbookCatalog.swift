import Combine
import Foundation
import OSLog

/// The lookbook catalog: ready-made sources you can install without leaving
/// Look, the way Obsidian lists community plugins.
///
/// Fetching lives here rather than in the engine because `URLSession` is where
/// every other network call in this app already happens, and the engine
/// gaining an HTTP client would make "local search and indexing never make
/// network calls" a footnote instead of a fact. Nothing here runs until
/// somebody opens the browse section: it is an explicit action, like
/// translation or the update check, not something that happens on launch.
@MainActor
final class LookbookCatalog: ObservableObject {
    static let shared = LookbookCatalog()

    /// One ready-made source, as the catalog describes it. Mirrors an entry in
    /// lookbook's `index.json`, which `make index` generates there.
    struct Entry: Decodable, Identifiable, Equatable {
        let id: String
        let name: String
        let description: String
        /// Where it works *as written*. An entry is listed on other platforms
        /// but cannot be installed there.
        let platforms: [String]
        /// Programs it cannot work without, usually because a helper script
        /// runs them and no block ever names them.
        let requires: [String]
        /// Declarations, copied into `~/.look/sources`.
        let files: [String]
        /// Helper scripts, copied into `~/.look/bin` and made executable.
        let bin: [String]
        let readme: String
        /// Where its files sit, relative to the catalog's base.
        let path: String
    }

    private struct Index: Decodable {
        let base: String
        /// The same files as pages a person can read. Kept apart from `base`,
        /// which serves raw bytes: right for a `.toml` about to be parsed,
        /// wrong for a README somebody clicked to read.
        let web: String?
        let sources: [Entry]
    }

    enum Reading: Equatable {
        case idle
        case loading
        case ready([Entry])
        case failed(String)
    }

    @Published private(set) var reading: Reading = .idle
    /// Programs from every entry's `requires` that this machine lacks, asked
    /// once per catalog rather than per card: the answer cannot change while
    /// the list is on screen, and a card redrawing per keystroke must not go
    /// back to disk for it.
    @Published private(set) var missing: Set<String> = []

    private var base = ""
    private var web = ""
    /// Counts probes. Each visit starts one and nothing orders them, so an
    /// older answer finishing late could put back a state the machine has
    /// already left. Only the newest may write `missing`.
    private var probeGeneration = 0
    private let session: URLSession
    private let log = Logger(subsystem: "noah-code.Look", category: "LookbookCatalog")

    private static let indexURL = URL(
        string: "https://raw.githubusercontent.com/kunkka19xx/lookbook/main/index.json"
    )!

    /// Points the browser at a checkout instead of GitHub, which is how this
    /// is worked on before a change to lookbook is merged. A path or a URL.
    private static let overrideKey = "LOOK_LOOKBOOK_INDEX"

    init(session: URLSession = .shared) {
        self.session = session
    }

    static var thisPlatform: String { "macos" }

    /// Fetches once and keeps the result for the life of the process. The
    /// catalog changes when somebody merges a PR to lookbook, which is not
    /// often enough to re-fetch every time this screen opens.
    func load(force: Bool = false) {
        if case .loading = reading { return }
        if case .ready(let entries) = reading, !force {
            // The catalog is worth fetching once. What is installed on this
            // machine is not: somebody can go and install docker between two
            // visits to this screen, and the Extend screen below re-checks
            // every time it opens. A card still saying "you do not have it"
            // would be the two disagreeing, which is the thing this is for.
            Task { await probeRequirements(of: entries) }
            return
        }
        reading = .loading
        Task { await fetch() }
    }

    private func fetch() async {
        let url = Self.resolvedURL()
        do {
            let data: Data
            if url.isFileURL {
                data = try Data(contentsOf: url)
            } else {
                var request = URLRequest(url: url)
                request.timeoutInterval = 15
                let (body, response) = try await session.data(for: request)
                guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                    reading = .failed("Could not reach lookbook.")
                    return
                }
                data = body
            }

            let index = try JSONDecoder().decode(Index.self, from: data)
            base = index.base
            // No fallback to `base`: that serves raw bytes, so the link
            // would open Markdown source in a browser, which is the thing
            // this button exists to avoid. A catalog without `web` simply
            // has no page to point at.
            web = index.web ?? ""
            reading = .ready(index.sources)
            await probeRequirements(of: index.sources)
        } catch {
            log.debug("lookbook catalog: \(error.localizedDescription, privacy: .public)")
            reading = .failed("Could not read the lookbook catalog.")
        }
    }

    /// One pass over every program any entry asks for, off the main thread
    /// because each name is a walk along `PATH`.
    private func probeRequirements(of entries: [Entry]) async {
        probeGeneration += 1
        let generation = probeGeneration

        let wanted = Array(Set(entries.flatMap(\.requires))).sorted()
        guard !wanted.isEmpty else {
            missing = []
            return
        }
        let absent = await Task.detached {
            EngineBridge.shared.lookbookMissing(programs: wanted)
        }.value
        guard generation == probeGeneration else { return }
        missing = Set(absent)
    }

    /// An entry's files, fetched in the order they will be written. Returns
    /// nil rather than a partial install: a declaration whose helper script
    /// failed to download is a row that does nothing.
    func fetchFiles(for entry: Entry) async -> LookbookInstall? {
        do {
            var declarations: [LookbookInstall.NamedFile] = []
            for name in entry.files {
                declarations.append(.init(name: name, contents: try await text(entry, name)))
            }
            var scripts: [LookbookInstall.NamedFile] = []
            for name in entry.bin {
                scripts.append(.init(name: name, contents: try await text(entry, "bin/\(name)")))
            }
            return LookbookInstall(declarations: declarations, scripts: scripts)
        } catch {
            log.debug("lookbook fetch: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    private func text(_ entry: Entry, _ name: String) async throws -> String {
        guard let url = Self.resolve(base: base, entry.path + name) else {
            throw URLError(.badURL)
        }
        let data: Data
        if url.isFileURL {
            data = try Data(contentsOf: url)
        } else {
            var request = URLRequest(url: url)
            request.timeoutInterval = 15
            let (body, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                throw URLError(.badServerResponse)
            }
            data = body
        }
        guard let text = String(data: data, encoding: .utf8) else {
            throw URLError(.cannotDecodeContentData)
        }
        return text
    }

    /// Where an entry's README can be read: the rendered page on GitHub, in a
    /// browser.
    ///
    /// Always the web page, even when the catalog is coming from a checkout.
    /// A local `.md` opens in whatever handles the extension, which is an
    /// editor showing the raw text, and the formatting is the reason someone
    /// clicked. Nil when the catalog publishes no `web`, so the card shows no
    /// button rather than one that opens Markdown source.
    func readme(for entry: Entry) -> URL? {
        guard !web.isEmpty else { return nil }
        return URL(string: web + entry.readme)
    }

    /// The directory the override points into, or nil when the catalog is
    /// coming over the network.
    private static func localRoot() -> URL? {
        guard let override = ProcessInfo.processInfo.environment[overrideKey],
            !override.isEmpty,
            URL(string: override)?.scheme == nil
        else {
            return nil
        }
        return URL(fileURLWithPath: override).deletingLastPathComponent()
    }

    private static func resolvedURL() -> URL {
        guard let override = ProcessInfo.processInfo.environment[overrideKey],
            !override.isEmpty
        else {
            return indexURL
        }
        return URL(string: override).flatMap { $0.scheme == nil ? nil : $0 }
            ?? URL(fileURLWithPath: override)
    }

    /// A relative path against the catalog's base, which is a checkout
    /// directory under the override and a raw.githubusercontent URL otherwise.
    ///
    /// Nil when the entry names something that is not a URL. It used to fall
    /// back to the index, which meant fetching `index.json` and installing it
    /// as somebody's declaration.
    private static func resolve(base: String, _ relative: String) -> URL? {
        if let local = localRoot() {
            return local.appendingPathComponent(relative)
        }
        return URL(string: base + relative)
    }
}
