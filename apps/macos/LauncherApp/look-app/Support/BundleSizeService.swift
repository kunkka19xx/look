import Foundation

/// The size of a bundle (`.app`), which on disk is a directory: its own
/// attributes report the directory entry, so the files inside are summed, the
/// way Finder's Get Info does.
enum BundleSizeService {
    private struct Entry {
        let modified: Date?
        let bytes: Int64
    }

    /// Keyed by path. A bundle is rewritten on update, which moves its
    /// modification date and invalidates the entry.
    private static var cache: [String: Entry] = [:]

    static func size(path: String) async -> Int64? {
        let modified = modificationDate(path: path)
        if let hit = cache[path], hit.modified == modified {
            return hit.bytes
        }
        guard let bytes = await sum(path: path) else { return nil }
        cache[path] = Entry(modified: modified, bytes: bytes)
        return bytes
    }

    private static func modificationDate(path: String) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
    }

    /// `@concurrent` is what moves the walk off the main actor: under
    /// approachable concurrency a plain `nonisolated async` runs on the
    /// caller's actor, and a large app would freeze the launcher for seconds.
    /// Still the caller's task, so moving to another row cancels it midway.
    @concurrent nonisolated private static func sum(path: String) async -> Int64? {
        sumSync(path: path)
    }

    nonisolated private static func sumSync(path: String) -> Int64? {
        let keys: Set<URLResourceKey> = [.isRegularFileKey, .totalFileSizeKey]
        guard
            let enumerator = FileManager.default.enumerator(
                at: URL(fileURLWithPath: path),
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
