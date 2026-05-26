import Foundation

struct FolderEntry: Equatable {
    let name: String
    let isDir: Bool
    let size: Int64?
}

struct FolderListing: Equatable {
    let items: [FolderEntry]
    let folderCount: Int
    let fileCount: Int
    let truncated: Bool
}

enum FolderListingService {
    nonisolated static let listCap = 30

    static func list(path: String) async -> FolderListing? {
        await Task.detached(priority: .userInitiated) {
            listSync(path: path)
        }.value
    }

    nonisolated private static func listSync(path: String) -> FolderListing? {
        let url = URL(fileURLWithPath: path)
        let keys: [URLResourceKey] = [.isDirectoryKey, .fileSizeKey]
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: url,
            includingPropertiesForKeys: keys,
            options: [.skipsHiddenFiles, .skipsSubdirectoryDescendants]
        ) else {
            return nil
        }

        var folderNames: [String] = []
        var files: [(name: String, url: URL)] = []

        for child in entries {
            let name = child.lastPathComponent
            if name.hasPrefix(".") { continue }
            let isDir = (try? child.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
            if isDir {
                folderNames.append(name)
            } else {
                files.append((name, child))
            }
        }

        folderNames.sort { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
        files.sort { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }

        let folderCount = folderNames.count
        let fileCount = files.count
        let total = folderCount + fileCount

        var items: [FolderEntry] = []
        items.reserveCapacity(min(total, listCap))

        for name in folderNames {
            if items.count >= listCap { break }
            items.append(FolderEntry(name: name, isDir: true, size: nil))
        }
        for file in files {
            if items.count >= listCap { break }
            let size = (try? file.url.resourceValues(forKeys: [.fileSizeKey]).fileSize).flatMap(Int64.init)
            items.append(FolderEntry(name: file.name, isDir: false, size: size))
        }

        return FolderListing(
            items: items,
            folderCount: folderCount,
            fileCount: fileCount,
            truncated: total > listCap
        )
    }
}
