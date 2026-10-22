import AppKit
import SwiftUI

/// Markdown drawn as blocks. `core/markdown` decides what a block is, so
/// every shell agrees; this decides how one looks. A fence is the same
/// `AICodeBlockView` an AI answer uses.
struct MarkdownBlocksView: View {
    let blocks: [MarkdownBlock]
    let themeStore: ThemeStore
    /// Lazy inside a scroll view: a 62 KB document is ~215 blocks and ~2000
    /// styled runs, each built on the main actor. A panel block stays eager.
    var lazy = false

    /// The panel's own body size (`ResultPreviewView.detailFont`).
    private static let clickable: Set<String> = ["http", "https", "mailto"]

    private var base: CGFloat { CGFloat(themeStore.settings.fontSize - 2) }

    var body: some View {
        if lazy {
            LazyVStack(alignment: .leading, spacing: 6) { rows }
        } else {
            VStack(alignment: .leading, spacing: 6) { rows }
        }
    }

    @ViewBuilder
    private var rows: some View {
        ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
            view(of: block)
        }
    }

    @ViewBuilder
    private func view(of block: MarkdownBlock) -> some View {
        switch block.kind {
        case "heading":
            Text(inline(block.spans ?? [], size: headingSize(block.level ?? 1), weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
                .textSelection(.enabled)
        case "paragraph":
            Text(inline(block.spans ?? [], size: base))
                .foregroundStyle(themeStore.secondaryTextColor())
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        case "list":
            VStack(alignment: .leading, spacing: 3) {
                ForEach(Array((block.items ?? []).enumerated()), id: \.offset) { index, item in
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(block.ordered == true ? "\((block.start ?? 1) + index)." : "\u{2022}")
                            .font(block.ordered == true ? mono(base) : themeStore.uiFont(size: base))
                            .foregroundStyle(themeStore.mutedTextColor())
                        Text(inline(item, size: base))
                            .foregroundStyle(themeStore.secondaryTextColor())
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                        Spacer(minLength: 0)
                    }
                }
            }
        case "code":
            AICodeBlockView(
                code: block.text ?? "",
                language: block.language,
                themeStore: themeStore,
                maxVisibleLines: 20
            )
        default:
            EmptyView()
        }
    }

    /// Headings reach body size by the third level: a glance surface carries
    /// the level on weight rather than spending the pane on a title.
    private func headingSize(_ level: Int) -> CGFloat {
        base + max(0, CGFloat(3 - level))
    }

    /// System monospaced, scaled like `uiFont`: the prose beside it zooms.
    private func mono(_ size: CGFloat) -> Font {
        .system(size: max(8, size * themeStore.uiScale), design: .monospaced)
    }

    private func inline(
        _ spans: [MarkdownSpan], size: CGFloat, weight: Font.Weight = .regular
    ) -> AttributedString {
        var out = AttributedString()
        for span in spans {
            var piece = AttributedString(span.text)
            switch span.kind {
            case "bold":
                piece.font = themeStore.uiFont(size: size, weight: .semibold)
            case "italic":
                piece.font = themeStore.uiFont(size: size, weight: weight).italic()
            case "code":
                piece.font = mono(size - 1)
                piece.foregroundColor = themeStore.accentColor()
            case "link":
                // Without a font it inherits whatever encloses it.
                piece.font = themeStore.uiFont(size: size, weight: weight)
                piece.foregroundColor = themeStore.accentColor()
                piece.underlineStyle = .single
                // The text shown is not the href, and a note is not trusted
                // input: `file://` and app schemes stay styled but inert.
                if let href = span.href, let url = URL(string: href),
                    Self.clickable.contains(url.scheme?.lowercased() ?? "")
                {
                    piece.link = url
                }
            default:
                piece.font = themeStore.uiFont(size: size, weight: weight)
            }
            out += piece
        }
        return out
    }
}

/// A markdown file's contents, parsed off the main thread.
struct MarkdownFilePreview: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let path: String
    var maxHeight: CGFloat = .infinity

    /// As `TextFilePreview`: long enough that arrowing through a folder of
    /// notes reads none of them.
    nonisolated private static let settleNanoseconds: UInt64 = 150_000_000

    @State private var blocks: [MarkdownBlock] = []

    var body: some View {
        ScrollView {
            MarkdownBlocksView(blocks: blocks, themeStore: themeStore, lazy: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(10)
        }
        .frame(maxWidth: .infinity, maxHeight: maxHeight)
        .background(
            themeStore.controlFillColor(),
            in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous)
        )
        .task(id: path) {
            let file = path
            // Cleared first, or the new row shows the previous file's text.
            // Then `TextFilePreview`'s debounce: a file arrowed past is
            // never read at all.
            blocks = []
            try? await Task.sleep(nanoseconds: Self.settleNanoseconds)
            if Task.isCancelled { return }
            let found = await Task.detached { () -> [MarkdownBlock] in
                guard let text = QuickLookPreviewService.head(ofFile: file) else { return [] }
                return EngineBridge.shared.markdownBlocks(text)
            }.value
            guard !Task.isCancelled else { return }
            blocks = found
        }
    }
}
