import Darwin
import Foundation

enum ConfigPathResolver {
    private static let productionBundleIdentifier = "noah-code.Look"

    static func resolvedPath() -> String {
        let env = ProcessInfo.processInfo.environment
        if let custom = env["LOOK_CONFIG_PATH"]?.trimmingCharacters(in: .whitespacesAndNewlines),
            !custom.isEmpty
        {
            return custom
        }

        let home = env["HOME"] ?? NSHomeDirectory()

        if let bundleIdentifier = Bundle.main.bundleIdentifier,
            bundleIdentifier.caseInsensitiveCompare(productionBundleIdentifier) != .orderedSame
        {
            return (home as NSString).appendingPathComponent(".look.dev.config")
        }

        let bundlePath = Bundle.main.bundleURL.resolvingSymlinksInPath().path.lowercased()
        if bundlePath.contains("/look dev.app") {
            return (home as NSString).appendingPathComponent(".look.dev.config")
        }

        return (home as NSString).appendingPathComponent(".look.config")
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
