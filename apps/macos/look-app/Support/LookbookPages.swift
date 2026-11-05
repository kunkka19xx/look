import Foundation

/// The rendered-page base for a lookbook catalog that serves raw bytes.
///
/// Out here rather than on `LookbookCatalog` so it can be tested: every card's
/// README link is built on it, and the published catalog declares no `web` of
/// its own, so a mistake here shows up as no link at all.
enum LookbookPages {
    /// `raw.githubusercontent.com/<owner>/<repo>/<ref>/` and
    /// `github.com/<owner>/<repo>/blob/<ref>/` address the same files, one as
    /// bytes and one as rendered pages, so the second is derivable from the
    /// first.
    ///
    /// Empty for anything else, a local checkout included: guessing a page URL
    /// for a host whose layout this does not know is a link to a 404.
    static func base(forRaw base: String) -> String {
        let raw = "https://raw.githubusercontent.com/"
        guard base.hasPrefix(raw) else { return "" }
        let parts = base.dropFirst(raw.count).split(separator: "/")
        // Owner, repo, ref. Anything after them is a directory inside the
        // repo, which the page URL keeps in the same place.
        guard parts.count >= 3 else { return "" }
        let blob = "https://github.com/\(parts[0])/\(parts[1])/blob/\(parts[2])/"
        let within = parts.dropFirst(3).joined(separator: "/")
        return within.isEmpty ? blob : blob + within + "/"
    }
}
