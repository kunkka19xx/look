import AppKit
import Foundation

/// Where `open -a Name` would find a bundle: the same roots `open` itself
/// searches, plus a bundle-id lookup for a name that is one.
enum AppBundleLocator {
    private static let roots = [
        "/Applications",
        "/System/Applications",
        "/System/Applications/Utilities",
        "/Applications/Utilities",
        NSHomeDirectory() + "/Applications",
    ]

    static func bundlePath(forAppNamed name: String) -> String? {
        let bundleName = name.hasSuffix(".app") ? name : name + ".app"
        for root in roots {
            let candidate = root + "/" + bundleName
            if FileManager.default.fileExists(atPath: candidate) {
                return candidate
            }
        }
        return NSWorkspace.shared
            .urlForApplication(withBundleIdentifier: name)?
            .path
    }
}
