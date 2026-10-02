import Foundation

/// Parsing and discovery logic for inline commands entered with either `/` or `:`.
/// Both prefixes are valid everywhere Look accepts commands (e.g. `/calc 2+2` or `:calc 2+2`).
enum InlineCommand {
    struct Match: Equatable {
        let id: String
        let args: String
        let hasSpace: Bool
    }

    /// Detects `:cmdid<space>...`, `/cmdid<space>...` (live trigger) and bare `:cmdid`, `/cmdid` (submit-only
    /// trigger). Returns nil if `input` doesn't begin with `:` or `/` or the id after
    /// the prefix (up to the first whitespace) isn't an exact match for a command in
    /// `catalog`. `hasSpace` distinguishes the two trigger paths.
    static func extract(from input: String, catalog: [AppCommand] = AppConstants.Launcher.commandCatalog) -> Match? {
        guard input.hasPrefix(":") || input.hasPrefix("/") else { return nil }
        let body = input.dropFirst()

        if let spaceIdx = body.firstIndex(where: { $0.isWhitespace }) {
            let id = String(body[..<spaceIdx]).lowercased()
            guard !id.isEmpty, catalog.contains(where: { $0.id == id }) else { return nil }
            let args = String(body[body.index(after: spaceIdx)...])
            return Match(id: id, args: args, hasSpace: true)
        }

        let id = String(body).lowercased()
        guard !id.isEmpty, catalog.contains(where: { $0.id == id }) else { return nil }
        return Match(id: id, args: "", hasSpace: false)
    }

    /// Whether this query should open the command discovery menu.
    /// Either `:` or `/` alone, or `:` / `/` followed by text that matches
    /// at least one command in the catalog. If text after the prefix has no matches,
    /// returns false so file path search (e.g. `/Users/...`) or normal search stays active.
    static func isDiscoveryQuery(input: String, catalog: [AppCommand] = AppConstants.Launcher.commandCatalog) -> Bool {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(":") || trimmed.hasPrefix("/") else { return false }
        if let match = extract(from: input, catalog: catalog), match.hasSpace { return false }
        if trimmed == ":" || trimmed == "/" { return true }
        let filter = String(trimmed.dropFirst())
        return !AppConstants.Launcher.commandCatalog(matching: filter).isEmpty
    }

    /// The filter term typed after `:` or `/`.
    static func filter(from input: String) -> String {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(":") || trimmed.hasPrefix("/") else { return "" }
        return String(trimmed.dropFirst())
    }
}
