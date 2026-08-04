import AppKit
import UniformTypeIdentifiers

/// Icon resolution for the synthesized calculator row. Mirrors
/// `LauncherProcessFeature.icon(forPID:)`: single source for the row and the
/// preview panel.
enum LauncherCalcFeature {
    /// SF Symbols has no calculator glyph, so borrow the real app's icon
    /// instead of an abstract stand-in.
    static func icon() -> NSImage {
        let path = AppConstants.Launcher.Calc.appIconPath
        return FileManager.default.fileExists(atPath: path)
            ? NSWorkspace.shared.icon(forFile: path)
            : NSImage(systemSymbolName: "number.square.fill", accessibilityDescription: nil)
                ?? NSWorkspace.shared.icon(for: .plainText)
    }
}
