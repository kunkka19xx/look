import Darwin
import Foundation

@_silgen_name("look_config_path")
nonisolated
private func look_config_path(_ dev: Bool) -> UnsafeMutablePointer<CChar>?

@_silgen_name("look_free_cstring")
nonisolated
private func look_free_config_cstring(_ ptr: UnsafeMutablePointer<CChar>?)

enum ConfigPathResolver {
    private static let productionBundleIdentifier = "noah-code.Look"

    /// Where this build reads and writes its settings.
    ///
    /// The app decides only WHICH build it is; the core decides where that
    /// build's file lives, including the one-time move of a legacy
    /// `~/.look.config` into `~/.look/`. Two implementations of "where is the
    /// config" could disagree, and the app would then read one file and save to
    /// another, which looks exactly like settings not persisting.
    static func resolvedPath() -> String {
        let env = ProcessInfo.processInfo.environment
        if let custom = env["LOOK_CONFIG_PATH"]?.trimmingCharacters(in: .whitespacesAndNewlines),
            !custom.isEmpty
        {
            // Ensured like every answer the core gives: an override into a
            // folder that does not exist yet is a path nothing can write.
            try? FileManager.default.createDirectory(
                atPath: (custom as NSString).deletingLastPathComponent,
                withIntermediateDirectories: true)
            return custom
        }

        let home = env["HOME"] ?? NSHomeDirectory()
        guard let ptr = look_config_path(isDevBuild) else {
            // The core could not answer (no HOME). Fall back to the legacy path
            // rather than losing the user's settings.
            return (home as NSString).appendingPathComponent(
                isDevBuild ? ".look/config.dev" : ".look.config")
        }
        defer { look_free_config_cstring(ptr) }
        return String(cString: ptr)
    }

    /// A dev build keeps its own settings, so running it never edits the
    /// installed copy's config.
    private static var isDevBuild: Bool {
        if let bundleIdentifier = Bundle.main.bundleIdentifier,
            bundleIdentifier.caseInsensitiveCompare(productionBundleIdentifier) != .orderedSame
        {
            return true
        }
        return Bundle.main.bundleURL
            .resolvingSymlinksInPath()
            .path
            .lowercased()
            .contains("/look dev.app")
    }

    static func applyDefaultConfigEnvironmentIfNeeded() {
        let env = ProcessInfo.processInfo.environment
        if let existing = env["LOOK_CONFIG_PATH"]?.trimmingCharacters(in: .whitespacesAndNewlines), !existing.isEmpty {
            return
        }

        let resolved = resolvedPath()
        setenv("LOOK_CONFIG_PATH", resolved, 1)
    }
}
