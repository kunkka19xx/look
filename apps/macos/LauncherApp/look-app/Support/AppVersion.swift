import Foundation

/// Single source of truth for the running app's version, read from the bundle
/// Info.plist. The CLI `-v` path in look_appApp has its own resilient reader for
/// the case where the binary is invoked outside the .app; inside the GUI we are
/// always running from the bundle, so Bundle.main is enough here.
enum AppVersion {
    /// Marketing version, e.g. "1.0.0". nil if missing from the plist.
    static var short: String? {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String)?
            .trimmingCharacters(in: .whitespaces)
            .nonEmpty
    }

    /// Build number, e.g. "42". nil if missing from the plist.
    static var build: String? {
        (Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String)?
            .trimmingCharacters(in: .whitespaces)
            .nonEmpty
    }

    /// True for side-by-side dev installs (`make app-run-dev` rewrites the
    /// bundle id to `noah-code.Look.Dev`). Dev builds carry a fixed 1.0 version,
    /// so comparing them against published releases is meaningless.
    static var isDevBuild: Bool {
        Bundle.main.bundleIdentifier?.hasSuffix(".Dev") ?? false
    }

    /// Human-readable label for display, e.g. "Look 1.0.0 (42)" or "Look 1.0.0".
    /// Dev builds are tagged so the version isn't mistaken for a real release.
    static var displayString: String {
        let suffix = isDevBuild ? " — dev" : ""
        guard let short else { return "Look (unknown version)\(suffix)" }
        if let build, build != short {
            return "Look \(short) (\(build))\(suffix)"
        }
        return "Look \(short)\(suffix)"
    }
}

private extension String {
    var nonEmpty: String? { isEmpty ? nil : self }
}
