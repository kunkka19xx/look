import AppKit
import SwiftUI

/// AppKit-backed display of large attributed strings. SwiftUI's `Text`
/// inside a ScrollView lays out the entire AttributedString
/// synchronously and slows badly with hundreds of color spans;
/// NSTextView uses TextKit's incremental layout and stays fast.
struct HighlightedTextView: NSViewRepresentable {
    let attributed: NSAttributedString
    let font: NSFont
    let defaultColor: NSColor

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = false
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        scroll.borderType = .noBorder

        let text = NSTextView()
        text.isEditable = false
        text.isSelectable = true
        text.drawsBackground = false
        text.textContainerInset = NSSize(width: 10, height: 10)
        text.textContainer?.lineFragmentPadding = 0
        text.isHorizontallyResizable = false
        text.isVerticallyResizable = true
        text.autoresizingMask = [.width]
        text.textContainer?.widthTracksTextView = true
        text.allowsUndo = false

        scroll.documentView = text
        return scroll
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        guard let text = nsView.documentView as? NSTextView,
              let storage = text.textStorage else { return }
        // Setting via textStorage avoids the perf cost of NSTextView's
        // `string` setter (which throws away typing attributes / undo).
        storage.setAttributedString(attributed)
        let fullRange = NSRange(location: 0, length: storage.length)
        // Apply default font over the whole range.
        storage.addAttribute(.font, value: font, range: fullRange)
        // Paint the theme's default text color on ranges the highlighter
        // didn't already color (identifiers, punctuation, whitespace).
        // NSTextView's default is labelColor - too bright for our pane.
        storage.enumerateAttribute(.foregroundColor, in: fullRange) { value, range, _ in
            if value == nil {
                storage.addAttribute(.foregroundColor, value: defaultColor, range: range)
            }
        }
        // Reset scroll to top when content changes.
        text.scroll(NSPoint(x: 0, y: 0))
    }
}
