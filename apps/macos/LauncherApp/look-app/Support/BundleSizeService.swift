import Foundation

/// The size of a bundle (`.app`), which on disk is a directory: its own
/// attributes report the directory entry, so the files inside are summed, the
/// way Finder's Get Info does.
enum BundleSizeService {
    private struct Stamp: Equatable {
        let bundle: Date?
        let infoPlist: Date?
    }

    private struct Entry {
        let stamp: Stamp
        let bytes: Int64
    }

    private static let infoPlistSubpath = "Contents/Info.plist"

    /// Keyed by path. Checked against the bundle's own date, which moves when
    /// an updater swaps the whole bundle, and its Info.plist's, which moves
    /// when one patches in place (the version is rewritten either way).
    private static var cache: [String: Entry] = [:]

    static func size(path: String) async -> Int64? {
        let root = URL(fileURLWithPath: path).resolvingSymlinksInPath()
        let stamp = Stamp(
            bundle: modificationDate(root),
            infoPlist: modificationDate(root.appendingPathComponent(infoPlistSubpath)))
        if let hit = cache[path], hit.stamp == stamp {
            return hit.bytes
        }
        guard let bytes = await sum(root: root) else { return nil }
        cache[path] = Entry(stamp: stamp, bytes: bytes)
        return bytes
    }

    private static func modificationDate(_ url: URL) -> Date? {
        (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
    }

    /// `@concurrent` is what moves the walk off the main actor: under
    /// approachable concurrency a plain `nonisolated async` runs on the
    /// caller's actor, and a large app would freeze the launcher for seconds.
    /// Still the caller's task, so moving to another row cancels it midway.
    @concurrent nonisolated private static func sum(root: URL) async -> Int64? {
        sumSync(root: root)
    }

    /// `root` must be symlink-resolved: enumerating a link (Safari.app, apps
    /// installed by Nix) yields nothing, and the bundle would read as empty.
    nonisolated private static func sumSync(root: URL) -> Int64? {
        let keys: Set<URLResourceKey> = [.isRegularFileKey, .totalFileSizeKey]
        guard
            let enumerator = FileManager.default.enumerator(
                at: root,
                includingPropertiesForKeys: Array(keys),
                options: [],
                errorHandler: nil)
        else { return nil }

        var total: Int64 = 0
        for case let url as URL in enumerator {
            if Task.isCancelled { return nil }
            guard let values = try? url.resourceValues(forKeys: keys),
                values.isRegularFile == true,
                let size = values.totalFileSize
            else { continue }
            total += Int64(size)
        }
        return total
    }
}
