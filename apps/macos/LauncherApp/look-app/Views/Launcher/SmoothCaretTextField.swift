import AppKit
import SwiftUI

/// The search field's editable input, with a "monkeytype" caret: the native
/// insertion point is suppressed and a bar is drawn that glides to the real caret
/// position (solid while typing, blinking when idle).
///
/// Built as an `NSTextField` subclass rather than a custom `NSTextView` so the
/// launcher's existing focus recovery (`findEditableTextField`, which looks for an
/// editable `NSTextField`) keeps finding and focusing it unchanged. The caret is
/// read from the field editor's layout, so it stays correct for any cursor
/// position, not just the end of the text.
struct SmoothCaretTextField: NSViewRepresentable {
    @Binding var text: String
    var placeholder: String
    var isFocused: FocusState<Bool>.Binding
    var font: NSFont
    var textColor: NSColor
    var caretColor: NSColor
    var onSubmit: () -> Void

    func makeNSView(context: Context) -> CaretTextField {
        let field = CaretTextField()
        field.delegate = context.coordinator
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.usesSingleLineMode = true
        field.cell?.isScrollable = true
        field.cell?.wraps = false
        field.lineBreakMode = .byClipping
        field.font = font
        field.textColor = textColor
        field.caretColor = caretColor
        field.setContentHuggingPriority(.defaultLow, for: .horizontal)
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        applyPlaceholder(to: field)
        return field
    }

    func updateNSView(_ field: CaretTextField, context: Context) {
        context.coordinator.parent = self

        if field.stringValue != text {
            field.stringValue = text
            field.refreshCaret(animated: true)
        }
        field.font = font
        field.textColor = textColor
        field.caretColor = caretColor
        applyPlaceholder(to: field)

        // Bridge SwiftUI focus INTO first responder, one direction only.
        // `currentEditor()` is non-nil exactly while the field is being edited, a
        // stable "focused" signal (the first-responder identity flickers during
        // layout and caused a makeFirstResponder feedback loop). Focus-in stops
        // firing as soon as editing begins, so it can't loop. Blur is left to
        // natural resignation (window hide / view swap) rather than a second
        // branch here, which would ping-pong against the async begin-editing sync.
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
            // Plain Return submits (Cmd+Return is consumed upstream by the
            // keyboard monitor, so it never reaches here). Consume it so the
            // field editor doesn't beep trying to insert a newline.
            if selector == #selector(NSResponder.insertNewline(_:)) {
                parent.onSubmit()
                return true
            }
            return false
        }
    }
}

/// The `NSTextField` that owns the caret bar. Kept file-internal to the smooth
/// caret; nothing else should instantiate it.
final class CaretTextField: NSTextField {
    var caretColor: NSColor = .labelColor {
        didSet { caretLayer.backgroundColor = caretColor.cgColor }
    }

    private let caretLayer = CALayer()
    private var selectionObserver: NSObjectProtocol?
    private var blinkResumeTimer: Timer?

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
        refreshCaret(animated: true)
    }

    override func textDidEndEditing(_ notification: Notification) {
        super.textDidEndEditing(notification)
        stopObservingSelection()
        blinkResumeTimer?.invalidate()
        caretLayer.removeAnimation(forKey: "blink")
        caretLayer.opacity = 0
    }

    // MARK: - Native caret suppression

    /// The field editor is shared per window, so clear the insertion point only
    /// while we own it; `textDidEndEditing` fires before another field edits, and
    /// each field editor session redraws its own caret, so no manual restore of
    /// the colour is needed.
    private func suppressNativeCaret() {
        (currentEditor() as? NSTextView)?.insertionPointColor = .clear
    }

    // MARK: - Caret geometry

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

        // Width of the text preceding the caret gives its x; an empty range at 0
        // yields a zero rect, so the caret sits at the leading edge.
        let precedingWidth = caretLocation > 0
            ? layoutManager.boundingRect(
                forGlyphRange: NSRange(location: 0, length: caretLocation),
                in: container
            ).maxX
            : 0

        let origin = editor.textContainerOrigin
        let xInEditor = origin.x + precedingWidth
        let xInField = editor.convert(CGPoint(x: xInEditor, y: 0), to: self).x

        let lineHeight = font.map { $0.ascender - $0.descender + $0.leading }
            ?? bounds.height
        let barHeight = lineHeight * Motion.Caret.heightScale
        let y = (bounds.height - barHeight) / 2

        return CGRect(x: xInField, y: y, width: Motion.Caret.width, height: barHeight)
    }

    // MARK: - Blink / solid-while-typing

    private func registerTyping() {
        caretLayer.removeAnimation(forKey: "blink")
        caretLayer.opacity = 1
        blinkResumeTimer?.invalidate()
        blinkResumeTimer = Timer.scheduledTimer(
            withTimeInterval: Motion.Caret.blinkResumeSeconds,
            repeats: false
        ) { [weak self] _ in
            self?.startBlink()
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
        caretLayer.add(blink, forKey: "blink")
    }

    // MARK: - Selection tracking

    private func observeSelection() {
        guard let editor = currentEditor() as? NSTextView else { return }
        selectionObserver = NotificationCenter.default.addObserver(
            forName: NSTextView.didChangeSelectionNotification,
            object: editor,
            queue: .main
        ) { [weak self] _ in
            self?.refreshCaret(animated: true)
        }
    }

    private func stopObservingSelection() {
        if let selectionObserver {
            NotificationCenter.default.removeObserver(selectionObserver)
            self.selectionObserver = nil
        }
    }
}
