import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ResultPreviewView: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let result: LauncherResult
    var onDeleteClipboard: (() -> Void)? = nil

    @State private var folderListing: FolderListing?
    @State private var trashItemCount: Int?

    /// The pinned Trash quick folder is TCC-protected, so it can't be listed
    /// like a normal folder — it gets a Finder-backed summary instead.
    private var isTrash: Bool {
        result.kind == .folder
            && DeleteTargetLogic.isTrashPath(result.path, homeDirectory: NSHomeDirectory())
    }

    private func folderCountText(_ listing: FolderListing) -> String? {
        var parts: [String] = []
        if listing.folderCount > 0 {
            parts.append("\(listing.folderCount) folder\(listing.folderCount == 1 ? "" : "s")")
        }
        if listing.fileCount > 0 {
            parts.append("\(listing.fileCount) file\(listing.fileCount == 1 ? "" : "s")")
        }
        return parts.isEmpty ? nil : parts.joined(separator: ", ")
    }

    private static let modifiedDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()
    private static let clipboardDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .medium
        return formatter
    }()

    private var clipboardIcon: NSImage {
        NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: nil)
            ?? NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)
            ?? NSWorkspace.shared.icon(for: .plainText)
    }

    private var largeIcon: NSImage {
        if result.id.hasPrefix("setting:") {
            let settingsPath = "/System/Applications/System Settings.app"
            if FileManager.default.fileExists(atPath: settingsPath) {
                return NSWorkspace.shared.icon(forFile: settingsPath)
            }
            let legacyPath = "/System/Applications/System Preferences.app"
            return NSWorkspace.shared.icon(forFile: legacyPath)
        }
        return NSWorkspace.shared.icon(forFile: result.path)
    }

    private var bundleInfo: (version: String?, size: String, modified: String?) {
        var version: String? = nil
        var modified: String? = nil
        var totalSize: Int64 = 0

        if result.id.hasPrefix("setting:") || result.kind == .app {
            let appPath = result.id.hasPrefix("setting:")
                ? "/System/Applications/System Settings.app"
                : result.path

            if let bundle = Bundle(path: appPath) {
                version = bundle.infoDictionary?["CFBundleShortVersionString"] as? String
                    ?? bundle.infoDictionary?["CFBundleVersion"] as? String
            }

            if let attrs = try? FileManager.default.attributesOfItem(atPath: appPath) {
                if let modDate = attrs[.modificationDate] as? Date {
                    modified = Self.modifiedDateFormatter.string(from: modDate)
                }
                if let size = attrs[.size] as? Int64 {
                    totalSize = size
                }
            }
        } else {
            if let attrs = try? FileManager.default.attributesOfItem(atPath: result.path) {
                if let size = attrs[.size] as? Int64 {
                    totalSize = size
                }
                if let modDate = attrs[.modificationDate] as? Date {
                    modified = Self.modifiedDateFormatter.string(from: modDate)
                }
            }
        }

        let sizeStr = formatFileSize(totalSize)
        return (version, sizeStr, modified)
    }

    private func formatFileSize(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }

    var body: some View {
        if result.kind == .clipboard {
            clipboardPreview
        } else {
        let info = bundleInfo

            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    Image(nsImage: largeIcon)
                        .resizable()
                        .frame(width: 48, height: 48)
                        .shadow(color: .black.opacity(0.3), radius: 4, x: 0, y: 2)

                    VStack(alignment: .leading, spacing: 4) {
                        Text(result.title)
                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: .semibold))
                            .foregroundStyle(themeStore.fontColor())
                            .lineLimit(2)

                        HStack(spacing: 6) {
                            KindBadge(kind: result.kind.rawValue)
                            if result.kind == .folder {
                                if isTrash {
                                    if let trashItemCount {
                                        Text("\(trashItemCount) item\(trashItemCount == 1 ? "" : "s")")
                                            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                            .foregroundStyle(themeStore.secondaryTextColor())
                                    }
                                } else if let listing = folderListing,
                                   let counts = folderCountText(listing) {
                                    Text(counts)
                                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                        .foregroundStyle(themeStore.secondaryTextColor())
                                }
                            } else {
                                Text(info.size)
                                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.secondaryTextColor())
                            }
                        }
                    }
                    Spacer()
                }

                if result.kind == .file {
                    if QuickLookPreviewService.isTextFile(path: result.path) {
                        TextFilePreview(path: result.path, maxHeight: .infinity)
                    } else {
                        QuickLookPreviewImage(path: result.path, maxHeight: .infinity)
                    }
                }

                if result.kind == .folder {
                    if isTrash {
                        TrashSummaryView(itemCount: trashItemCount, themeStore: themeStore)
                    } else {
                        FolderPreviewView(path: result.path, listing: folderListing)
                    }
                }

                if let version = info.version {
                    InfoRow(label: "Version", value: version)
                }

                InfoRow(label: "Kind", value: result.kind.rawValue.capitalized)

                VStack(alignment: .leading, spacing: 2) {
                    Text("Path")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                    Text(result.path)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                        .lineLimit(3)
                }

                if let modified = info.modified {
                    InfoRow(label: "Modified", value: modified)
                }

                if result.kind != .file && result.kind != .folder {
                    Spacer()
                }
            }
            .padding(12)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .task(id: result.kind == .folder ? result.path : "") {
                guard result.kind == .folder else {
                    folderListing = nil
                    trashItemCount = nil
                    return
                }
                if isTrash {
                    // Don't list ~/.Trash (TCC) and don't prompt for Automation
                    // just by previewing — only show a count if already granted.
                    folderListing = nil
                    trashItemCount = EmptyTrashCommand.itemCount(promptIfNeeded: false)
                    return
                }
                folderListing = nil
                let path = result.path
                let listing = await FolderListingService.list(path: path)
                // .task(id:) cancels this closure when the result changes,
                // but the detached worker keeps running — guard against
                // stale assignment when the user moved on to another folder.
                if Task.isCancelled { return }
                folderListing = listing
            }
        }
    }

    private var clipboardPreview: some View {
        let content = result.clipboardContent ?? ""
        let capturedAt = result.clipboardCapturedAt.map { Self.clipboardDateFormatter.string(from: $0) } ?? "Unknown"
        let characterCount = result.clipboardCharacterCount ?? content.count
        let lineCount = result.clipboardLineCount ?? max(1, content.split(whereSeparator: \.isNewline).count)

        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Image(nsImage: clipboardIcon)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 34, height: 34)
                    .foregroundStyle(themeStore.accentColor())
                VStack(alignment: .leading, spacing: 2) {
                    Text("Clipboard item")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 1), weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                    Text("Captured \(capturedAt)")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
                Spacer()

                if let onDeleteClipboard {
                    Button {
                        onDeleteClipboard()
                    } label: {
                        Label("Delete", systemImage: "trash")
                    }
                    .buttonStyle(.plain)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                    .foregroundStyle(themeStore.dangerColor().opacity(0.95))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(themeStore.dangerColor().opacity(0.16), in: Capsule())
                }
            }

            HStack(spacing: 8) {
                KindBadge(kind: "clipboard")
                Text("\(characterCount) chars")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
                Text("\(lineCount) lines")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }

            Text("Preview")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .medium))
                .foregroundStyle(themeStore.mutedTextColor())

            ScrollView {
                Text(content)
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular, design: .monospaced))
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                    .textSelection(.enabled)
                    .padding(10)
            }
            .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 8, style: .continuous))

            InfoRow(label: "Kind", value: "Clipboard")
            InfoRow(label: "Captured", value: capturedAt)

            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

struct KindBadge: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let kind: String

    private var color: Color {
        switch kind {
        case "app": return themeStore.accentColor()
        case "file": return themeStore.successColor()
        case "folder": return themeStore.warningColor()
        case "clipboard": return themeStore.accentColor()
        default: return themeStore.mutedTextColor()
        }
    }

    private var foreground: Color {
        switch kind {
        case "file":
            return themeStore.onSuccessColor()
        case "folder":
            return themeStore.onWarningColor()
        default:
            return themeStore.onAccentColor()
        }
    }

    var body: some View {
        Text(kind.capitalized)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium))
            .foregroundStyle(foreground)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.8), in: Capsule())
    }
}

struct InfoRow: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
            Spacer()
            Text(value)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())
        }
    }
}
