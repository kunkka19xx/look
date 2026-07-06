import AppKit
import SwiftUI

/// In-process text-file preview: reads the first chunk of a text/code
/// file off the main thread, syntax-highlights it, and displays it in
/// an AppKit NSTextView (much faster than SwiftUI Text for large
/// attributed strings).
///
/// The file is already size-capped upstream (see
/// `QuickLookPreviewService.sizeCap`); this view additionally caps the
/// displayed content at 16 KB.
struct TextFilePreview: View {
    @EnvironmentObject private var themeStore: ThemeStore
    let path: String
    let maxHeight: CGFloat

    // NSTextView (used by HighlightedTextView) virtualizes layout via
    // TextKit, so we can comfortably show larger files than SwiftUI
    // Text allowed. 64 KB is ≈2000 lines - well over preview needs.
    nonisolated private static let displayByteCap: Int = 64 * 1024
    // NSAttributedString isn't Sendable but is documented thread-safe
    // for read access; mark the static empty constant nonisolated(unsafe).
    nonisolated(unsafe) private static let empty = NSAttributedString(string: "")

    nonisolated private final class HighlightResult: @unchecked Sendable {
        let attr: NSAttributedString
        let key: String?
        init(_ attr: NSAttributedString, _ key: String?) {
            self.attr = attr
            self.key = key
        }
    }

    @State private var content: NSAttributedString = TextFilePreview.empty

    private var displayFont: NSFont {
        NSFont.monospacedSystemFont(
            ofSize: CGFloat(themeStore.settings.fontSize - 1),
            weight: .regular
        )
    }

    var body: some View {
        HighlightedTextView(
            attributed: content,
            font: displayFont,
            defaultColor: NSColor(themeStore.secondaryTextColor())
        )
            .frame(maxWidth: .infinity, maxHeight: maxHeight)
            .background(themeStore.controlFillColor(),
                        in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .task(id: path) {
                let p = path
                // Cache hit → paint immediately, no debounce / no I/O.
                if let attrs = try? FileManager.default.attributesOfItem(atPath: p),
                   let mtime = attrs[.modificationDate] as? Date,
                   let size = attrs[.size] as? Int64,
                   let cached = HighlightedTextCache.get(
                       HighlightedTextCache.key(path: p, mtime: mtime, size: size)
                   ) {
                    content = cached
                    return
                }

                content = Self.empty
                try? await Task.sleep(nanoseconds: 150_000_000)
                if Task.isCancelled { return }

                let result = await Task.detached { () -> HighlightResult in
                    guard let data = try? Data(
                        contentsOf: URL(fileURLWithPath: p),
                        options: [.mappedIfSafe]
                    ) else { return HighlightResult(Self.empty, nil) }
                    let slice = data.prefix(Self.displayByteCap)
                    // Lossy UTF-8 decode: never fails on partial codepoints
                    // or invalid bytes (becomes U+FFFD). The Latin-1
                    // fallback was wrong for UTF-8 files truncated
                    // mid-codepoint and produced mojibake for non-ASCII.
                    let text = String(decoding: slice, as: UTF8.self)
                    let highlighted = SyntaxHighlighter.highlight(text, path: p)
                    let attrs = try? FileManager.default.attributesOfItem(atPath: p)
                    let mtime = attrs?[.modificationDate] as? Date
                    let size = attrs?[.size] as? Int64
                    let keyStr: String? = (mtime != nil && size != nil)
                        ? HighlightedTextCache.key(path: p, mtime: mtime!, size: size!)
                        : nil
                    return HighlightResult(highlighted, keyStr)
                }.value
                if Task.isCancelled { return }
                content = result.attr
                if let keyStr = result.key {
                    HighlightedTextCache.set(keyStr, result.attr)
                }
            }
    }
}
