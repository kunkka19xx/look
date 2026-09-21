import AppKit
import Carbon.HIToolbox
import OSLog

/// Types ⌘V into whichever app owns the cursor once Look is out of the way.
enum FrontmostAppPaste {
    enum Blocker {
        case accessibilityDenied
        case secureInput

        var banner: String {
            switch self {
            case .accessibilityDenied:
                AppConstants.Launcher.Clipboard.accessibilityDeniedBanner
            case .secureInput:
                AppConstants.Launcher.Clipboard.secureInputBanner
            }
        }
    }

    private enum Timing {
        /// Reactivation after a hide is asynchronous, so the target is polled
        /// rather than guessed at: a keystroke sent early lands nowhere.
        static let poll: Duration = .milliseconds(15)
        static let activationTimeout: TimeInterval = 0.6
        static let modifierReleaseTimeout: TimeInterval = 1.0
        /// A freshly activated window still has to give some text view first
        /// responder before it holds a cursor.
        static let settle: Duration = .milliseconds(40)
        /// How long the clip stays on the pasteboard after ⌘V is posted.
        static let pasteboardHandback: Duration = .milliseconds(500)
    }

    /// The value of `kAXTrustedCheckOptionPrompt`, spelled out: the constant is
    /// a global `var`, which Swift 6 refuses to read.
    private static let accessibilityPromptOptionKey = "AXTrustedCheckOptionPrompt"
    /// Virtual key codes past this are modifiers and function keys.
    private static let maxScannedKeyCode: UInt16 = 128
    private static let translatedCharacterCapacity = 4

    nonisolated private static let logger = Logger(subsystem: "noah-code.Look", category: "paste")

    static func blocker() -> Blocker? {
        // Secure input (a password field, `sudo` in a terminal) drops
        // synthesized events without a trace, so it is worth saying.
        if IsSecureEventInputEnabled() { return .secureInput }
        if !AXIsProcessTrusted() { return .accessibilityDenied }
        return nil
    }

    /// macOS shows this once per app per session, so it is safe to call on
    /// every refused paste.
    static func requestAccessibilityPermission() {
        _ = AXIsProcessTrustedWithOptions([accessibilityPromptOptionKey: true] as CFDictionary)
    }

    /// Posts ⌘V once `pid` owns the keyboard, nil meaning whatever takes focus
    /// after Look leaves. `pasted` runs when the target has had time to read the
    /// pasteboard, and is skipped when nothing was pasted, so the clip stays
    /// there for a ⌘V by hand.
    static func paste(into pid: pid_t?, pasted: (@MainActor () -> Void)? = nil) {
        Task { @MainActor in
            let deadline = Date().addingTimeInterval(Timing.activationTimeout)
            while !targetHasFocus(pid) {
                guard Date() < deadline else {
                    logger.notice("paste: target never took focus")
                    return
                }
                try? await Task.sleep(for: Timing.poll)
            }
            await waitForModifierRelease()
            try? await Task.sleep(for: Timing.settle)
            postCommandV()
            guard let pasted else { return }
            try? await Task.sleep(for: Timing.pasteboardHandback)
            pasted()
        }
    }

    private static func targetHasFocus(_ pid: pid_t?) -> Bool {
        guard let frontmost = NSWorkspace.shared.frontmostApplication,
            frontmost.processIdentifier != ProcessInfo.processInfo.processIdentifier
        else { return false }
        guard let pid else { return true }
        return frontmost.processIdentifier == pid
    }

    /// ⌘ is usually still down from ⌘I. Pasting under it works, but the physical
    /// release then lands after the synthetic key-up, and an app reading the
    /// chord as a whole sees one ⌘ press spanning both.
    private static func waitForModifierRelease() async {
        let deadline = Date().addingTimeInterval(Timing.modifierReleaseTimeout)
        while modifiersAreHeld() {
            guard Date() < deadline else {
                logger.notice("paste: modifiers still held, pasting anyway")
                return
            }
            try? await Task.sleep(for: Timing.poll)
        }
    }

    private static func modifiersAreHeld() -> Bool {
        let held = CGEventSource.flagsState(.combinedSessionState)
        return !held.intersection([.maskCommand, .maskShift, .maskAlternate, .maskControl])
            .isEmpty
    }

    private static func postCommandV() {
        guard let source = CGEventSource(stateID: .combinedSessionState) else {
            logger.notice("paste: no event source")
            return
        }
        let key = pasteKeyCode()
        guard let down = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: true),
            let up = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: false)
        else {
            logger.notice("paste: could not build the key events")
            return
        }
        down.flags = .maskCommand
        up.flags = .maskCommand
        down.post(tap: .cgAnnotatedSessionEventTap)
        up.post(tap: .cgAnnotatedSessionEventTap)
    }

    /// The key that types "v" on the active layout. `kVK_ANSI_V` is a physical
    /// position, which types another letter on Dvorak/Colemak, and the receiving
    /// app matches ⌘V by character.
    private static func pasteKeyCode() -> CGKeyCode {
        guard let layout = currentKeyboardLayout() else { return CGKeyCode(kVK_ANSI_V) }
        for candidate in 0..<maxScannedKeyCode
        where character(for: candidate, layout: layout) == "v" {
            return CGKeyCode(candidate)
        }
        return CGKeyCode(kVK_ANSI_V)
    }

    private static func currentKeyboardLayout() -> Data? {
        guard let source = TISCopyCurrentASCIICapableKeyboardLayoutInputSource()?
            .takeRetainedValue(),
            let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
        else { return nil }
        return Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue() as Data
    }

    private static func character(for keyCode: UInt16, layout: Data) -> String? {
        var deadKeyState: UInt32 = 0
        var length = 0
        var characters = [UniChar](repeating: 0, count: translatedCharacterCapacity)
        let status = layout.withUnsafeBytes { bytes -> OSStatus in
            guard let base = bytes.baseAddress else { return OSStatus(paramErr) }
            return UCKeyTranslate(
                base.assumingMemoryBound(to: UCKeyboardLayout.self),
                keyCode,
                UInt16(kUCKeyActionDown),
                0,
                UInt32(LMGetKbdType()),
                OptionBits(kUCKeyTranslateNoDeadKeysBit),
                &deadKeyState,
                translatedCharacterCapacity,
                &length,
                &characters
            )
        }
        guard status == noErr, length > 0 else { return nil }
        return String(utf16CodeUnits: characters, count: length)
    }
}
