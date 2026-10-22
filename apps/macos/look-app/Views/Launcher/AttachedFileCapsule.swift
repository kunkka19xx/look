import AppKit
import SwiftUI

/// A file `@`-attached to a turn, shown under the message it was asked about.
/// Clicking reveals it in Finder rather than opening it: the question was about
/// the contents, so "where is this" is the useful follow-up, and revealing
/// cannot launch anything unexpected.
struct AttachedFileCapsule: View {
    let path: String
    let themeStore: ThemeStore

    @State private var hovering = false

    private var name: String { PathDisplay.name(of: path) }
    private var directory: String { PathDisplay.directory(of: path) }

    var body: some View {
        Button {
            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "doc.text")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 4)))
                VStack(alignment: .leading, spacing: 0) {
                    Text(name)
                        .font(
                            themeStore.uiFont(
                                size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium)
                        )
                        .lineLimit(1)
                    // Which `main.go`. A transcript outlives the moment it was
                    // written in, and the name alone stops identifying the file
                    // as soon as a second one shares it. Head-truncated, so the
                    // folder nearest the file survives.
                    if !directory.isEmpty {
                        Text(directory)
                            .font(
                                themeStore.uiFont(
                                    size: CGFloat(themeStore.settings.fontSize - 5), weight: .regular)
                            )
                            .foregroundStyle(themeStore.mutedTextColor())
                            .lineLimit(1)
                            .truncationMode(.head)
                    }
                }
                Image(systemName: "arrow.up.forward.app")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 5)))
                    .opacity(hovering ? 0.9 : 0.35)
            }
            .foregroundStyle(themeStore.fontColor())
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                themeStore.accentColor().opacity(hovering ? 0.22 : 0.14),
                in: RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Reveal \(path) in Finder")
    }
}
