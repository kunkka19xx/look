import AppKit
import SwiftUI

/// The search field's editable input, with a gliding caret (solid while typing,
/// blinking when idle). An `NSTextField` subclass, not a custom `NSTextView`, so
/// the launcher's existing focus recovery (`findEditableTextField`, which looks
/// for an editable `NSTextField`) keeps working unchanged.
struct SmoothCaretTextField: NSViewRepresentable {
    /// How tall the input may grow before it scrolls instead. Six lines is a
    /// composer, not an editor: past that the launcher would swallow the panel
    /// it is meant to sit above.
    private static let maxVisibleLines = 6

    @Binding var text: String
    var placeholder: String
    var isFocused: FocusState<Bool>.Binding
    var themeStore: ThemeStore
    /// Overrides the base theme font size when set (the Todo search bar runs a
    /// touch larger). Colors and family always follow the theme.
    var fontSize: CGFloat? = nil
    /// Lets Shift+Return insert a line break and the field wrap and grow. Only
    /// AI mode asks for it: the search bar is a single line by design, and a
    /// query with a newline in it means nothing to the matcher.
    var allowsMultiline: Bool = false
    var onSubmit: () -> Void

    private var font: NSFont { themeStore.uiNSFont(size: fontSize) }
    private var textColor: NSColor { NSColor(themeStore.fontColor()) }
    private var caretColor: NSColor { NSColor(themeStore.accentColor()) }
    private var lineHeight: CGFloat { font.ascender - font.descender + font.leading }

    func makeNSView(context: Context) -> CaretTextField {
        let field = CaretTextField()
        field.delegate = context.coordinator
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = font
        field.textColor = textColor
        field.caretColor = caretColor
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        applyLineMode(to: field)
        context.coordinator.appliedMultiline = allowsMultiline
        applyPlaceholder(to: field)
        return field
    }

    /// Single line clips and scrolls sideways; multiline wraps at the field's
    /// width and grows until `sizeThatFits` caps it.
    private func applyLineMode(to field: CaretTextField) {
        field.usesSingleLineMode = !allowsMultiline
        field.lineBreakMode = allowsMultiline ? .byWordWrapping : .byClipping
        // `wraps` and `isScrollable` are mutually exclusive in NSCell: setting
        // either one clears the other. Assign ONLY the one this mode wants, or
        // the second assignment silently undoes the first.
        if allowsMultiline {
            field.cell?.wraps = true
        } else {
            field.cell?.isScrollable = true
        }
    }

    func sizeThatFits(
        _ proposal: ProposedViewSize, nsView field: CaretTextField, context: Context
    ) -> CGSize? {
        // nil means "size me the way you always did" - the single-line path is
        // untouched.
        guard allowsMultiline, let cell = field.cell else { return nil }
        guard let width = proposal.width, width.isFinite, width > 0 else { return nil }

        let unbounded = NSRect(x: 0, y: 0, width: width, height: .greatestFiniteMagnitude)
        let wrapped = cell.cellSize(forBounds: unbounded).height
        let cap = lineHeight * CGFloat(Self.maxVisibleLines)
        return CGSize(width: width, height: min(max(wrapped, lineHeight), cap))
    }

    func updateNSView(_ field: CaretTextField, context: Context) {
        context.coordinator.parent = self

        if context.coordinator.appliedMultiline != allowsMultiline {
            context.coordinator.appliedMultiline = allowsMultiline
            applyLineMode(to: field)
            // The field editor is configured from the cell when editing BEGINS,
            // so a live edit would keep the old line mode until it restarts.
            // Restart it here, putting the caret back where the user left it.
            if let window = field.window, let editor = field.currentEditor() {
                let selection = editor.selectedRange
                window.makeFirstResponder(nil)
                window.makeFirstResponder(field)
                field.currentEditor()?.selectedRange = selection
                field.refreshCaret(animated: false)
            }
        }

        if field.stringValue != text {
            field.stringValue = text
            // A programmatic set (recall, clear) otherwise drops the caret to the
            // start; move it to the end so the recalled text is editable at once.
            field.currentEditor()?.selectedRange = NSRange(location: (text as NSString).length, length: 0)
            // Settling: a recalled prompt can be several lines, and the field
            // has not grown to fit them at this point.
            field.refreshCaretSettling()
        }
        field.font = font
        field.textColor = textColor
        field.caretColor = caretColor
        applyPlaceholder(to: field)

        // Bridge focus INTO first responder only. `currentEditor() != nil` is a
        // stable "editing" signal (unlike first-responder identity, which flickers
        // during layout and loops makeFirstResponder). Blur is left to natural
        // resignation (window hide / view swap), avoiding a two-way ping-pong.
        let isEditing = field.currentEditor() != nil
        if isFocused.wrappedValue, !isEditing, field.window != nil {
            DispatchQueue.main.async {
                guard field.currentEditor() == nil, field.window != nil else { return }
                field.window?.makeFirstResponder(field)
            }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    private func applyPlaceholder(to field: CaretTextField) {
        field.placeholderAttributedString = NSAttributedString(
            string: placeholder,
            attributes: [
                .foregroundColor: NSColor.secondaryLabelColor,
                .font: font,
            ]
        )
    }

    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: SmoothCaretTextField
        /// The line mode currently applied to the field, so a mode flip is
        /// detected once rather than re-applied on every SwiftUI update.
        var appliedMultiline = false

        init(_ parent: SmoothCaretTextField) { self.parent = parent }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? CaretTextField else { return }
            parent.text = field.stringValue
        }

        func controlTextDidBeginEditing(_ obj: Notification) {
            DispatchQueue.main.async { self.parent.isFocused.wrappedValue = true }
        }

        func controlTextDidEndEditing(_ obj: Notification) {
            DispatchQueue.main.async { self.parent.isFocused.wrappedValue = false }
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
            let isReturn = selector == #selector(NSResponder.insertNewline(_:))
            let isSoftReturn = selector == #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:))
            guard isReturn || isSoftReturn else { return false }

            // A field editor routes Shift+Return to insertNewline: too, so the
            // SELECTOR cannot tell a send from a line break - only the event
            // can. Reading the selector alone sent the message instead.
            let shiftHeld = NSApp.currentEvent.map {
                $0.type == .keyDown && $0.modifierFlags.contains(.shift)
            } ?? false

            if parent.allowsMultiline, isSoftReturn || shiftHeld {
                textView.insertText("\n", replacementRange: textView.selectedRange())
                // insertText posts the change notification, but the binding is
                // what the submit path reads: set it here so a send that lands
                // in the same runloop turn cannot miss the newline.
                parent.text = textView.string
                // Past the height cap the box stops growing, so the new line
                // has to be brought into view rather than left below the edge.
                textView.scrollRangeToVisible(textView.selectedRange())
                // Settling, not immediate: the line this break just created is
                // not laid out yet (see refreshCaretSettling).
                (control as? CaretTextField)?.refreshCaretSettling()
                return true
            }

            // Plain Return submits (Cmd+Return is consumed upstream by the
            // keyboard monitor, so it never reaches here). Both forms are
            // consumed either way, so the field editor never beeps.
            if isReturn { parent.onSubmit() }
            return true
        }
    }
}

/// The `NSTextField` that owns the caret bar. Kept file-internal to the smooth
/// caret; nothing else should instantiate it.
final class CaretTextField: NSTextField {
    var caretColor: NSColor = .labelColor {
        didSet { caretLayer.backgroundColor = caretColor.cgColor }
    }

    private static let blinkKey = "blink"
    /// One layout pass away, matching the launcher's other "let AppKit settle"
    /// staged delays. Short enough that the correction is not seen as a move.
    private static let caretSettleSeconds = 0.04

    private let caretLayer = CALayer()
    private var selectionObserver: NSObjectProtocol?
    private var blinkResumeTimer: Timer?
    /// The shared field editor we suppressed, and the caret color it had before,
    /// so we can restore it on end-editing (the editor is reused by other native
    /// text fields in the same window).
    private weak var suppressedEditor: NSTextView?
    private var savedInsertionPointColor: NSColor?

    private static let glideTiming: CAMediaTimingFunction = {
        let c = Motion.houseCurveControlPoints
        return CAMediaTimingFunction(controlPoints: c.0, c.1, c.2, c.3)
    }()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        configureCaretLayer()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        configureCaretLayer()
    }

    private func configureCaretLayer() {
        wantsLayer = true
        caretLayer.backgroundColor = caretColor.cgColor
        caretLayer.cornerRadius = Motion.Caret.cornerRadius
        caretLayer.opacity = 0
        caretLayer.zPosition = 1
        layer?.addSublayer(caretLayer)
    }

    // MARK: - Editing lifecycle

    override func textDidBeginEditing(_ notification: Notification) {
        super.textDidBeginEditing(notification)
        suppressNativeCaret()
        observeSelection()
        caretLayer.opacity = 1
        refreshCaret(animated: false)
        registerTyping()
    }

    override func textDidChange(_ notification: Notification) {
        super.textDidChange(notification)
        registerTyping()
        refreshCaretSettling()
    }

    override func textDidEndEditing(_ notification: Notification) {
        super.textDidEndEditing(notification)
        restoreNativeCaret()
        stopObservingSelection()
        blinkResumeTimer?.invalidate()
        caretLayer.removeAnimation(forKey: Self.blinkKey)
        caretLayer.opacity = 0
    }

    // MARK: - Native caret suppression

    /// Hides the native insertion point on the shared window field editor while we
    /// draw our own, remembering the editor and its prior color.
    private func suppressNativeCaret() {
        guard let editor = currentEditor() as? NSTextView else { return }
        suppressedEditor = editor
        savedInsertionPointColor = editor.insertionPointColor
        editor.insertionPointColor = .clear
    }

    /// Restores the field editor's caret color on end-editing, so other native
    /// text fields in the window (Pomo name, Todo drafts) keep a visible caret.
    private func restoreNativeCaret() {
        suppressedEditor?.insertionPointColor = savedInsertionPointColor ?? .textColor
        suppressedEditor = nil
        savedInsertionPointColor = nil
    }

    // MARK: - Caret geometry

    /// Measure now, then once more after layout has settled. A change that adds
    /// a LINE (Shift+Return, or a word wrapping onto the next one) arrives here
    /// before the field editor has grown and laid that line out, so the
    /// immediate pass measures the old last line and parks the bar one line up
    /// until the next keystroke corrects it. The second pass is what lands it.
    func refreshCaretSettling() {
        refreshCaret(animated: true)
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.caretSettleSeconds) { [weak self] in
            self?.refreshCaret(animated: true)
        }
    }

    /// Recomputes the bar's frame from the field editor's layout and moves it,
    /// gliding when `animated`.
    func refreshCaret(animated: Bool) {
        guard window?.firstResponder === currentEditor(), let rect = caretRect() else { return }
        CATransaction.begin()
        if animated {
            CATransaction.setAnimationDuration(Motion.Caret.glideSeconds)
            CATransaction.setAnimationTimingFunction(Self.glideTiming)
        } else {
            CATransaction.setDisableActions(true)
        }
        caretLayer.frame = rect
        CATransaction.commit()
    }

    private func caretRect() -> CGRect? {
        guard
            let editor = currentEditor() as? NSTextView,
            let layoutManager = editor.layoutManager,
            let container = editor.textContainer
        else { return nil }

        let caretLocation = editor.selectedRange().location
        layoutManager.ensureLayout(for: container)

        let fontLineHeight = font.map { $0.ascender - $0.descender + $0.leading }
            ?? bounds.height
        let barHeight = fontLineHeight * Motion.Caret.heightScale
        let origin = editor.textContainerOrigin

        // Ask the text view where the insertion point is. This is the same
        // answer input methods get, so it is right for the cases the layout
        // manager makes awkward - notably a caret parked after a trailing
        // newline, which lives in a fragment that holds no glyphs.
        if let insertionLine = insertionPointLine(in: editor, at: caretLocation) {
            return CGRect(
                x: insertionLine.minX, y: insertionLine.midY - barHeight / 2,
                width: Motion.Caret.width, height: barHeight)
        }

        guard let line = lineGeometry(
            at: caretLocation, layoutManager: layoutManager, container: container, editor: editor)
        else {
            // Nothing laid out yet (empty field): leading edge, centred.
            let xInField = editor.convert(CGPoint(x: origin.x, y: 0), to: self).x
            return CGRect(
                x: xInField, y: (bounds.height - barHeight) / 2,
                width: Motion.Caret.width, height: barHeight)
        }

        // Convert as a RECT, not a point: the field editor is flipped and the
        // field is not, so only a rect conversion gets both axes right when the
        // caret is on the second or third line.
        let inField = editor.convert(
            CGRect(
                x: origin.x + line.x, y: origin.y + line.rect.minY,
                width: Motion.Caret.width, height: line.rect.height),
            to: self)

        return CGRect(
            x: inField.minX, y: inField.midY - barHeight / 2,
            width: Motion.Caret.width, height: barHeight)
    }

    /// The insertion point's line, in this field's coordinates, via the text
    /// input protocol. Returns nil when the view is off screen or the rect
    /// comes back degenerate, so the layout-manager path can still answer.
    private func insertionPointLine(in editor: NSTextView, at location: Int) -> CGRect? {
        guard let window else { return nil }
        let onScreen = editor.firstRect(
            forCharacterRange: NSRange(location: location, length: 0), actualRange: nil)
        guard onScreen.height > 0 else { return nil }
        return convert(window.convertFromScreen(onScreen), from: nil)
    }

    /// Where the caret sits: the fragment rect of its line, plus the x offset
    /// within that line. Returns nil when there is nothing laid out to measure.
    private func lineGeometry(
        at location: Int, layoutManager: NSLayoutManager, container: NSTextContainer,
        editor: NSTextView
    ) -> (rect: CGRect, x: CGFloat)? {
        let length = (editor.string as NSString).length
        let caretLocation = max(0, min(location, length))

        let glyphCount = layoutManager.numberOfGlyphs

        // A caret parked after a trailing newline (or in an empty field) lives
        // in the extra fragment, which holds no glyphs of its own. Falling
        // through to the last glyph would put the bar on the newline character,
        // which sits at the END of the PREVIOUS line - the bug this guards.
        if caretLocation == length, length == 0 || editor.string.hasSuffix("\n") {
            let extra = layoutManager.extraLineFragmentRect
            if extra.height > 0 { return (extra, extra.minX) }
            guard glyphCount > 0 else { return nil }
            // No extra fragment laid out: step one line down from the last one.
            let last = layoutManager.lineFragmentRect(
                forGlyphAt: glyphCount - 1, effectiveRange: nil)
            return (last.offsetBy(dx: 0, dy: last.height), last.minX)
        }

        guard glyphCount > 0 else { return nil }

        let caretGlyph = min(layoutManager.glyphIndexForCharacter(at: caretLocation), glyphCount)
        var lineGlyphRange = NSRange(location: 0, length: 0)
        let lineRect = layoutManager.lineFragmentRect(
            forGlyphAt: min(caretGlyph, glyphCount - 1), effectiveRange: &lineGlyphRange)

        // Width of this LINE's text up to the caret. Measuring from glyph 0
        // instead would push the caret off the right edge on every line but the
        // first.
        let precedingLength = caretGlyph - lineGlyphRange.location
        let x = precedingLength > 0
            ? layoutManager.boundingRect(
                forGlyphRange: NSRange(location: lineGlyphRange.location, length: precedingLength),
                in: container
            ).maxX
            : lineRect.minX

        return (lineRect, x)
    }

    // MARK: - Blink / solid-while-typing

    private func registerTyping() {
        caretLayer.removeAnimation(forKey: Self.blinkKey)
        caretLayer.opacity = 1
        blinkResumeTimer?.invalidate()
        blinkResumeTimer = Timer.scheduledTimer(
            withTimeInterval: Motion.Caret.blinkResumeSeconds,
            repeats: false
        ) { [weak self] _ in
            // Timer fires on RunLoop.main; assumeIsolated avoids a needless
            // Task hop while satisfying Swift 6's Sendable-closure check.
            MainActor.assumeIsolated {
                self?.startBlink()
            }
        }
    }

    private func startBlink() {
        guard window?.firstResponder === currentEditor() else { return }
        let blink = CAKeyframeAnimation(keyPath: "opacity")
        blink.values = [1.0, 1.0, 0.0, 0.0, 1.0]
        blink.keyTimes = [0.0, 0.45, 0.5, 0.95, 1.0]
        blink.duration = Motion.Caret.blinkPeriodSeconds
        blink.repeatCount = .infinity
        blink.calculationMode = .cubic
        caretLayer.add(blink, forKey: Self.blinkKey)
    }

    // MARK: - Selection tracking

    private func observeSelection() {
        guard let editor = currentEditor() as? NSTextView else { return }
        selectionObserver = NotificationCenter.default.addObserver(
            forName: NSTextView.didChangeSelectionNotification,
            object: editor,
            queue: .main
        ) { [weak self] _ in
            // Delivered on `queue: .main`, so the caret can move without a hop
            // that would land it a frame behind the selection.
            MainActor.assumeIsolated {
                self?.refreshCaret(animated: true)
            }
        }
    }

    private func stopObservingSelection() {
        if let selectionObserver {
            NotificationCenter.default.removeObserver(selectionObserver)
            self.selectionObserver = nil
        }
    }
}
