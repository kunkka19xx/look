import AppKit
import Carbon
import Combine
import OSLog

nonisolated private let launcherHotkeyLog = Logger(subsystem: "noah-code.Look", category: "hotkey")

/// `launcher_hotkey` as resolved by core (`look_launcher_hotkey_json`). Core owns
/// the grammar and the fallback; this side only turns it into Carbon terms.
nonisolated struct LauncherHotkeySpec: Decodable {
    struct Hotkey: Decodable {
        let modifiers: [String]
        let key: Key
    }

    struct Key: Decodable {
        let code: String
        let character: String?
    }

    let hotkey: Hotkey
    let display: String
    let defaultSpec: String
    let warning: String?

    enum CodingKeys: String, CodingKey {
        case hotkey, display, warning
        case defaultSpec = "default_spec"
    }
}

/// A spec checked by core (`look_hotkey_check_json`): canonical spelling plus
/// either how to show it or why it is rejected.
nonisolated struct HotkeyCheck: Decodable {
    let spec: String
    let display: String?
    let error: String?
}

/// A hotkey in the two forms macOS needs: Carbon values for the global
/// registration, and `NSEvent` values for the in-app monitor.
struct CarbonHotkey: Equatable {
    let keyCode: UInt32
    let carbonModifiers: UInt32
    let eventModifiers: NSEvent.ModifierFlags
    let display: String

    static let modifierMask: NSEvent.ModifierFlags = [.command, .control, .option, .shift]

    static let fallback = CarbonHotkey(
        keyCode: UInt32(kVK_Space),
        carbonModifiers: UInt32(cmdKey),
        eventModifiers: .command,
        display: "Cmd+Space"
    )

    init(keyCode: UInt32, carbonModifiers: UInt32, eventModifiers: NSEvent.ModifierFlags, display: String) {
        self.keyCode = keyCode
        self.carbonModifiers = carbonModifiers
        self.eventModifiers = eventModifiers
        self.display = display
    }

    init?(spec: LauncherHotkeySpec) {
        guard let keyCode = LauncherHotkeyKeyCodes.resolve(spec.hotkey.key) else {
            return nil
        }
        var carbon: UInt32 = 0
        var flags: NSEvent.ModifierFlags = []
        for name in spec.hotkey.modifiers {
            guard let modifier = Self.modifiers[name] else {
                return nil
            }
            carbon |= modifier.carbon
            flags.insert(modifier.flag)
        }
        self.init(keyCode: UInt32(keyCode), carbonModifiers: carbon, eventModifiers: flags, display: spec.display)
    }

    func matches(_ event: NSEvent) -> Bool {
        UInt32(event.keyCode) == keyCode
            && event.modifierFlags.intersection(Self.modifierMask) == eventModifiers
    }

    /// Keyed by core's modifier names, which its grammar also accepts as input.
    private static let modifiers: [String: (carbon: UInt32, flag: NSEvent.ModifierFlags)] = [
        "command": (UInt32(cmdKey), .command),
        "control": (UInt32(controlKey), .control),
        "option": (UInt32(optionKey), .option),
        "shift": (UInt32(shiftKey), .shift),
    ]

    /// The config spec for a pressed key, in core's grammar, for core to check.
    static func spec(for event: NSEvent) -> String? {
        guard let key = LauncherHotkeyKeyCodes.token(for: event) else {
            return nil
        }
        let flags = event.modifierFlags.intersection(modifierMask)
        let names = modifiers
            .filter { flags.contains($0.value.flag) }
            .map(\.key)
            .sorted()
        return (names + [key]).joined(separator: specSeparator)
    }

    private static let specSeparator = "+"
}

/// Maps core's W3C key codes to macOS virtual key codes.
///
/// Printable keys are looked up by the character they type on the user's
/// ASCII-capable layout, so `cmd+z` means the key labelled Z on AZERTY or QWERTZ
/// too. The US position is the fallback when the layout has no such key.
enum LauncherHotkeyKeyCodes {
    private static let maxVirtualKeyCode: UInt16 = 127
    private static let translatedCharacterCapacity = 4

    /// The grammar token for a pressed key: the code name for a fixed key, what
    /// the key types unshifted for any other.
    static func token(for event: NSEvent) -> String? {
        let keyCode = Int(event.keyCode)
        if let code = fixedKeys.first(where: { $0.value == keyCode })?.key {
            return code.lowercased()
        }
        guard !keypadKeys.contains(keyCode),
            let typed = event.characters(byApplyingModifiers: []),
            !typed.isEmpty
        else {
            return nil
        }
        return typed.lowercased()
    }

    static func resolve(_ key: LauncherHotkeySpec.Key) -> Int? {
        if let fixed = fixedKeys[key.code] {
            return fixed
        }
        let usPosition = usLayoutKeys[key.code]
        guard let character = key.character else {
            return usPosition
        }
        return keyCodeOnCurrentLayout(typing: character, preferring: usPosition) ?? usPosition
    }

    private static func keyCodeOnCurrentLayout(typing character: String, preferring preferred: Int?) -> Int? {
        guard let source = TISCopyCurrentASCIICapableKeyboardLayoutInputSource()?.takeRetainedValue(),
            let rawLayout = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
        else {
            return nil
        }
        let layoutData = Unmanaged<CFData>.fromOpaque(rawLayout).takeUnretainedValue() as Data
        let target = character.lowercased()

        return layoutData.withUnsafeBytes { buffer -> Int? in
            guard let layout = buffer.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else {
                return nil
            }
            if let preferred, translate(UInt16(preferred), layout: layout) == target {
                return preferred
            }
            for keyCode in 0...maxVirtualKeyCode where !keypadKeys.contains(Int(keyCode)) {
                if translate(keyCode, layout: layout) == target {
                    return Int(keyCode)
                }
            }
            return nil
        }
    }

    private static func translate(_ keyCode: UInt16, layout: UnsafePointer<UCKeyboardLayout>) -> String? {
        var deadKeyState: UInt32 = 0
        var length = 0
        var characters = [UniChar](repeating: 0, count: translatedCharacterCapacity)
        let status = UCKeyTranslate(
            layout,
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
        guard status == noErr, length > 0 else {
            return nil
        }
        return String(utf16CodeUnits: characters, count: length).lowercased()
    }

    /// Keys whose position does not depend on the layout.
    private static let fixedKeys: [String: Int] = [
        "Space": kVK_Space,
        "Enter": kVK_Return,
        "Tab": kVK_Tab,
        "Escape": kVK_Escape,
        "Backspace": kVK_Delete,
        "Delete": kVK_ForwardDelete,
        "ArrowUp": kVK_UpArrow,
        "ArrowDown": kVK_DownArrow,
        "ArrowLeft": kVK_LeftArrow,
        "ArrowRight": kVK_RightArrow,
        "Home": kVK_Home,
        "End": kVK_End,
        "PageUp": kVK_PageUp,
        "PageDown": kVK_PageDown,
        "F1": kVK_F1, "F2": kVK_F2, "F3": kVK_F3, "F4": kVK_F4, "F5": kVK_F5,
        "F6": kVK_F6, "F7": kVK_F7, "F8": kVK_F8, "F9": kVK_F9, "F10": kVK_F10,
        "F11": kVK_F11, "F12": kVK_F12, "F13": kVK_F13, "F14": kVK_F14, "F15": kVK_F15,
        "F16": kVK_F16, "F17": kVK_F17, "F18": kVK_F18, "F19": kVK_F19, "F20": kVK_F20,
    ]

    private static let usLayoutKeys: [String: Int] = [
        "KeyA": kVK_ANSI_A, "KeyB": kVK_ANSI_B, "KeyC": kVK_ANSI_C, "KeyD": kVK_ANSI_D,
        "KeyE": kVK_ANSI_E, "KeyF": kVK_ANSI_F, "KeyG": kVK_ANSI_G, "KeyH": kVK_ANSI_H,
        "KeyI": kVK_ANSI_I, "KeyJ": kVK_ANSI_J, "KeyK": kVK_ANSI_K, "KeyL": kVK_ANSI_L,
        "KeyM": kVK_ANSI_M, "KeyN": kVK_ANSI_N, "KeyO": kVK_ANSI_O, "KeyP": kVK_ANSI_P,
        "KeyQ": kVK_ANSI_Q, "KeyR": kVK_ANSI_R, "KeyS": kVK_ANSI_S, "KeyT": kVK_ANSI_T,
        "KeyU": kVK_ANSI_U, "KeyV": kVK_ANSI_V, "KeyW": kVK_ANSI_W, "KeyX": kVK_ANSI_X,
        "KeyY": kVK_ANSI_Y, "KeyZ": kVK_ANSI_Z,
        "Digit0": kVK_ANSI_0, "Digit1": kVK_ANSI_1, "Digit2": kVK_ANSI_2, "Digit3": kVK_ANSI_3,
        "Digit4": kVK_ANSI_4, "Digit5": kVK_ANSI_5, "Digit6": kVK_ANSI_6, "Digit7": kVK_ANSI_7,
        "Digit8": kVK_ANSI_8, "Digit9": kVK_ANSI_9,
        "Backquote": kVK_ANSI_Grave,
        "Minus": kVK_ANSI_Minus,
        "Equal": kVK_ANSI_Equal,
        "BracketLeft": kVK_ANSI_LeftBracket,
        "BracketRight": kVK_ANSI_RightBracket,
        "Backslash": kVK_ANSI_Backslash,
        "Semicolon": kVK_ANSI_Semicolon,
        "Quote": kVK_ANSI_Quote,
        "Comma": kVK_ANSI_Comma,
        "Period": kVK_ANSI_Period,
        "Slash": kVK_ANSI_Slash,
    ]

    /// The keypad types digits and symbols too, but a hotkey on `1` means the
    /// main row.
    private static let keypadKeys: Set<Int> = [
        kVK_ANSI_KeypadDecimal, kVK_ANSI_KeypadMultiply, kVK_ANSI_KeypadPlus,
        kVK_ANSI_KeypadClear, kVK_ANSI_KeypadDivide, kVK_ANSI_KeypadEnter,
        kVK_ANSI_KeypadMinus, kVK_ANSI_KeypadEquals,
        kVK_ANSI_Keypad0, kVK_ANSI_Keypad1, kVK_ANSI_Keypad2, kVK_ANSI_Keypad3,
        kVK_ANSI_Keypad4, kVK_ANSI_Keypad5, kVK_ANSI_Keypad6, kVK_ANSI_Keypad7,
        kVK_ANSI_Keypad8, kVK_ANSI_Keypad9,
    ]
}

/// Owns the launcher toggle registration so launch and config reload apply
/// `launcher_hotkey` the same way.
@MainActor
final class LauncherHotkeyController: ObservableObject {
    static let shared = LauncherHotkeyController()

    private let manager = GlobalHotKeyManager()

    /// What is registered right now, for screens that show the binding.
    @Published private(set) var display = CarbonHotkey.fallback.display
    @Published private(set) var defaultSpec: String?

    /// Releases the hotkey so a shortcut recorder can receive it as a plain key
    /// press. `reload()` takes it back.
    func suspend() {
        manager.suspend()
    }

    /// Registers the hotkey the config names and returns why it was not
    /// honoured, when it was not.
    @discardableResult
    func reload() -> String? {
        // A recorder is listening; it reloads when it stops.
        guard !ShortcutCapture.isActive else { return nil }
        let warning = apply()
        if let warning {
            launcherHotkeyLog.error("\(warning, privacy: .public)")
        }
        return warning
    }

    private func apply() -> String? {
        guard let spec = EngineBridge.shared.launcherHotkey() else {
            register(.fallback)
            return nil
        }
        defaultSpec = spec.defaultSpec
        guard let hotkey = CarbonHotkey(spec: spec) else {
            register(.fallback)
            return "\(spec.display) has no key on this keyboard layout. Using \(CarbonHotkey.fallback.display)"
        }
        register(hotkey)
        return spec.warning
    }

    private func register(_ hotkey: CarbonHotkey) {
        manager.registerToggleHotKey(hotkey)
        display = hotkey.display
    }
}
