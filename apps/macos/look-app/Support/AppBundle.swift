import Darwin
import Foundation

/// The .app this executable lives in, found from the executable's real path.
/// Run through a symlink such as Homebrew's `lookapp`, `Bundle.main` resolves
/// to the link's directory and carries no Info.plist or bundle identifier.
nonisolated enum AppBundle {
    static let url: URL? = enclosingApp(of: executableURL())

    static let bundle: Bundle = url.flatMap(Bundle.init(url:)) ?? .main

    static var identifier: String? { bundle.bundleIdentifier }

    /// True when started by a path outside the bundle, so `Bundle.main` is wrong
    /// and this process must not become the app.
    static var isLaunchedOutsideBundle: Bool {
        guard let url else { return false }
        return Bundle.main.bundleURL.resolvingSymlinksInPath() != url
    }

    private static func executableURL() -> URL? {
        var size: UInt32 = 0
        _ = _NSGetExecutablePath(nil, &size)
        guard size > 0 else { return nil }
        var buffer = [CChar](repeating: 0, count: Int(size))
        guard _NSGetExecutablePath(&buffer, &size) == 0 else { return nil }
        let bytes = buffer.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }
        return URL(fileURLWithPath: String(decoding: bytes, as: UTF8.self)).resolvingSymlinksInPath()
    }

    private static func enclosingApp(of executable: URL?) -> URL? {
        guard var cursor = executable?.deletingLastPathComponent() else { return nil }
        while cursor.pathComponents.count > 1 {
            if cursor.pathExtension == "app" { return cursor }
            cursor = cursor.deletingLastPathComponent()
        }
        return nil
    }
}
