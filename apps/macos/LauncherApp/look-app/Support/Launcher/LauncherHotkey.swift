import AppKit
import Carbon
import Combine
import OSLog

nonisolated private let launcherHotkeyLog = Logger(subsystem: "noah-code.Look", category: "hotkey")

/// `launcher_hotkey` as resolved by core, which owns the grammar and fallback.
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
    let enabled: Bool
    let display: String
    let defaultSpec: String
    let warning: String?

    enum CodingKeys: String, CodingKey {
        case hotkey, enabled, display, warning
        case defaultSpec = "default_spec"
    }
}

nonisolated struct HotkeyCheck: Decodable {
    let spec: String
    let display: String?
    let error: String?
}

/// Carbon values for the global registration, `NSEvent` ones for the in-app monitor.
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

    /// Keyed by core's modifier names, which its grammar also accepts.
    fileprivate static let modifiers: [String: (carbon: Int, flag: NSEvent.ModifierFlags)] = [
        "command": (cmdKey, .command),
        "control": (controlKey, .control),
        "option": (optionKey, .option),
        "shift": (shiftKey, .shift),
    ]

    func matches(_ event: NSEvent) -> Bool {
        UInt32(event.keyCode) == keyCode
            && event.modifierFlags.intersection(Self.modifierMask) == eventModifiers
    }

    /// A pressed key in core's grammar, for core to check.
    static func spec(for event: NSEvent) -> String? {
        guard let key = HotkeyKeyCodes.token(for: event) else { return nil }
        let flags = event.modifierFlags
        let names = modifiers.filter { flags.contains($0.value.flag) }.map(\.key)
        return (names + [key]).joined(separator: "+")
    }
}

extension CarbonHotkey {
    init?(spec: LauncherHotkeySpec) {
        guard let keyCode = HotkeyKeyCodes.resolve(spec.hotkey.key) else { return nil }
        var carbon = 0
        var flags: NSEvent.ModifierFlags = []
        for name in spec.hotkey.modifiers {
            guard let modifier = Self.modifiers[name] else { return nil }
            carbon |= modifier.carbon
            flags.insert(modifier.flag)
        }
        self.init(
            keyCode: UInt32(keyCode), carbonModifiers: UInt32(carbon), eventModifiers: flags,
            display: spec.display)
    }
}

/// Printable keys are found by the character they type on the ASCII-capable
/// layout, so `cmd+z` means the key labelled Z on AZERTY too.
enum HotkeyKeyCodes {
    private static let maxKeyCode: UInt16 = 127
    private static let characterCapacity = 4

    private static let fixedKeys: [String: Int] = [
        "Space": kVK_Space, "Enter": kVK_Return, "Tab": kVK_Tab, "Escape": kVK_Escape,
        "F1": kVK_F1, "F2": kVK_F2, "F3": kVK_F3, "F4": kVK_F4, "F5": kVK_F5,
        "F6": kVK_F6, "F7": kVK_F7, "F8": kVK_F8, "F9": kVK_F9, "F10": kVK_F10,
        "F11": kVK_F11, "F12": kVK_F12, "F13": kVK_F13, "F14": kVK_F14, "F15": kVK_F15,
        "F16": kVK_F16, "F17": kVK_F17, "F18": kVK_F18, "F19": kVK_F19, "F20": kVK_F20,
    ]

    static func token(for event: NSEvent) -> String? {
        if let code = fixedKeys.first(where: { $0.value == Int(event.keyCode) })?.key {
            return code.lowercased()
        }
        return event.characters(byApplyingModifiers: [])?.lowercased()
    }

    static func resolve(_ key: LauncherHotkeySpec.Key) -> Int? {
        if let fixed = fixedKeys[key.code] { return fixed }
        guard let character = key.character?.lowercased(),
            let source = TISCopyCurrentASCIICapableKeyboardLayoutInputSource()?.takeRetainedValue(),
            let rawLayout = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
        else {
            return nil
        }
        let data = Unmanaged<CFData>.fromOpaque(rawLayout).takeUnretainedValue() as Data
        return data.withUnsafeBytes { buffer -> Int? in
            guard let layout = buffer.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else {
                return nil
            }
            // Ascending, so the main row wins over the keypad.
            return (0...maxKeyCode).first { translate($0, layout) == character }.map(Int.init)
        }
    }

    private static func translate(_ keyCode: UInt16, _ layout: UnsafePointer<UCKeyboardLayout>) -> String? {
        var deadKeyState: UInt32 = 0
        var length = 0
        var chars = [UniChar](repeating: 0, count: characterCapacity)
        let status = UCKeyTranslate(
            layout, keyCode, UInt16(kUCKeyActionDown), 0, UInt32(LMGetKbdType()),
            OptionBits(kUCKeyTranslateNoDeadKeysBit), &deadKeyState, characterCapacity, &length, &chars)
        guard status == noErr, length > 0 else { return nil }
        return String(utf16CodeUnits: chars, count: length).lowercased()
    }
}

/// Applies `launcher_hotkey` at launch and on config reload.
@MainActor
final class LauncherHotkeyController: ObservableObject, ShortcutRegistration {
    static let shared = LauncherHotkeyController()

    private let manager = GlobalHotKeyManager()

    @Published private(set) var display = CarbonHotkey.fallback.display
    @Published private(set) var defaultSpec: String?

    func suspend() {
        manager.suspend()
    }

    /// Returns why the configured value was not honoured, if it was not.
    @discardableResult
    func reload() -> String? {
        // A listening recorder reloads when it stops.
        guard !ShortcutCapture.isActive else { return nil }
        let warning = apply()
        if let warning {
            launcherHotkeyLog.error("\(warning, privacy: .public)")
        }
        return warning
    }

    private func apply() -> String? {
        guard let spec = EngineBridge.shared.launcherHotkey() else {
            return register(.fallback)
        }
        defaultSpec = spec.defaultSpec
        guard spec.enabled else {
            manager.suspend()
            display = spec.display
            return nil
        }
        guard let hotkey = CarbonHotkey(spec: spec) else {
            let unusable =
                "\(spec.display) has no key on this keyboard layout. Using \(CarbonHotkey.fallback.display)"
            return [unusable, register(.fallback)].compactMap { $0 }.joined(separator: ". ")
        }
        return [spec.warning, register(hotkey)].compactMap { $0 }.joined(separator: ". ").nilIfEmpty
    }

    /// Returns why the key is dead, when Carbon refused it: another app owns it.
    /// Retries continue in the background.
    private func register(_ hotkey: CarbonHotkey) -> String? {
        let status = manager.registerToggleHotKey(hotkey)
        display = hotkey.display
        guard status != noErr else { return nil }
        return "\(hotkey.display) is taken by another app, so it will not open Look"
    }
}

extension String {
    fileprivate var nilIfEmpty: String? { isEmpty ? nil : self }
}
