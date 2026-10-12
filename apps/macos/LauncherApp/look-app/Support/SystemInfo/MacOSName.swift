import Foundation

/// Pure macOS version-codename mapping used by the System Info panel.
///
/// Apple jumped the macOS version number from 15 (Sequoia) straight to 26
/// (Tahoe). Known names are mapped explicitly. Anything not yet known (a
/// future major) falls back to plain "macOS", and the caller appends the
/// version number, so the panel never shows a wrong OS name.
enum MacOSName {
    static let legacyFallback = "macOS"

    static func forMajor(_ major: Int) -> String {
        switch major {
        case 15:
            return "Sequoia"
        case 26:
            return "Tahoe"
        case 14:
            return "Sonoma"
        case 13:
            return "Ventura"
        case 12:
            return "Monterey"
        case 11:
            return "Big Sur"
        default:
            // The caller appends the full version number, so an unknown
            // major shows as "macOS 27.0.1" rather than a guessed name.
            return legacyFallback
        }
    }
}
