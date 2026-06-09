import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct FolderPreviewView: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let path: String
    let listing: FolderListing?

    private var rowFont: Font {
        themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular)
    }

    private var sizeFont: Font {
        themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular)
    }

    private func formatSize(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }

    private func icon(for entry: FolderEntry) -> NSImage {
        let childPath = (path as NSString).appendingPathComponent(entry.name)
        return NSWorkspace.shared.icon(forFile: childPath)
    }

    private func open(_ entry: FolderEntry) {
        let childPath = (path as NSString).appendingPathComponent(entry.name)
        NSWorkspace.shared.open(URL(fileURLWithPath: childPath))
    }

    var body: some View {
        if let listing {
            if listing.items.isEmpty {
                Text("Empty folder")
                    .font(rowFont)
                    .foregroundStyle(themeStore.mutedTextColor())
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 16)
            } else {
                list(listing)
            }
        }
    }

    @ViewBuilder
    private func list(_ listing: FolderListing) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(listing.items.enumerated()), id: \.offset) { index, entry in
                    if index > 0,
                       entry.isDir == false,
                       listing.items[index - 1].isDir == true {
                        Divider()
                            .padding(.vertical, 4)
                    }
                    row(entry)
                }

                if listing.truncated {
                    Text("Showing \(listing.items.count) of \(listing.folderCount + listing.fileCount)")
                        .font(sizeFont)
                        .foregroundStyle(themeStore.mutedTextColor())
                        .padding(.vertical, 6)
                        .frame(maxWidth: .infinity, alignment: .center)
                }
            }
        }
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .frame(maxHeight: .infinity)
    }

    @ViewBuilder
    private func row(_ entry: FolderEntry) -> some View {
        HStack(spacing: 8) {
            Image(nsImage: icon(for: entry))
                .resizable()
                .frame(width: 16, height: 16)

            Text(entry.name)
                .font(rowFont)
                .foregroundStyle(themeStore.fontColor())
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer(minLength: 8)

            if !entry.isDir, let size = entry.size {
                Text(formatSize(size))
                    .font(sizeFont)
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
        }
        .contentShape(Rectangle())
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .onTapGesture { open(entry) }
    }
}

/// Finder-backed summary shown in place of a normal folder listing for the
/// Trash quick folder, which is TCC-protected and can't be enumerated directly.
/// `itemCount` is nil until Finder automation is granted (we never prompt just
/// to preview), so the card stays informative without demanding permission.
struct TrashSummaryView: View {
    let itemCount: Int?
    let themeStore: ThemeStore

    private var trashIcon: NSImage {
        NSImage(named: NSImage.trashFullName)
            ?? NSWorkspace.shared.icon(for: .folder)
    }

    private var headline: String {
        switch itemCount {
        case .none: return "Open in Finder to view contents"
        case .some(0): return "Trash is empty"
        case .some(let count): return "\(count) item\(count == 1 ? "" : "s") in Trash"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Image(nsImage: trashIcon)
                    .resizable()
                    .frame(width: 28, height: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text(headline)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                        .foregroundStyle(themeStore.fontColor())
                    Text("⌘D empties the Trash")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
                Spacer()
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(themeStore.controlFillColor().opacity(0.4), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}
