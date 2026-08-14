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

    private var name: String { (path as NSString).lastPathComponent }

    var body: some View {
        Button {
            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "doc.text")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 4)))
                Text(name)
                    .font(
                        themeStore.uiFont(
                            size: CGFloat(themeStore.settings.fontSize - 3), weight: .medium)
                    )
                    .lineLimit(1)
                Image(systemName: "arrow.up.forward.app")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 5)))
                    .opacity(hovering ? 0.9 : 0.35)
            }
            .foregroundStyle(themeStore.fontColor())
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .background(
                themeStore.accentColor().opacity(hovering ? 0.22 : 0.14),
                in: Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Reveal \(path) in Finder")
    }
}
