import AppKit
import Foundation

/// Screen-based auto-scale and placement for the launcher window.
///
/// Mirrors the Linux/Windows formula in
/// apps/linows/src-tauri/src/main.rs (`scaled_window_size`):
/// 1.0× at ≤1080-point screen height, linear to 1.2× at 1440p,
/// capped at 1.3× on taller displays.
///
/// SwiftUI/AppKit work in points, so unlike the Tauri version we do
/// not multiply by backingScaleFactor - the OS handles that.
/// NSScreen height on a 2× Retina 4K is already 1080 points, matching
/// the Tauri `logical_h = physical_h / scale` derivation.
///
/// The launcher is not draggable: it opens at a fixed
/// position on whichever screen holds the mouse cursor, recomputed on
/// every show (see `LauncherView.toggleWindowVisibility`).
enum WindowAutoScale {
    // Base size matches the Linux/Windows build (apps/linows/src-tauri):
    // 860×580 logical, landscape - list pane + preview pane side by side.
    static let baseWidth: CGFloat = 860
    static let baseHeight: CGFloat = 600

    /// Extra points to lift the launcher above vertical center. The window is
    /// centered on the screen (middle - height/2), then raised by this so the
    /// search bar sits a little above center like Spotlight. Absolute, so
    /// placement is consistent on any display size or orientation.
    static let spotlightLift: CGFloat = 25

    static func ratio(forScreenHeightPoints h: CGFloat) -> CGFloat {
        guard h > 1080 else { return 1.0 }
        let r = 1.0 + (h - 1080) / (1440 - 1080) * 0.2
        return min(r, 1.3)
    }

    /// Base (unscaled) size of the launcher window. Running apps render inside
    /// the search bar, so the window is always the bordered-panel size.
    static func baseSize() -> CGSize {
        CGSize(width: baseWidth, height: baseHeight)
    }

    /// Window size for the given screen: the base panel multiplied by the
    /// screen ratio.
    static func size(for screen: NSScreen) -> CGSize {
        let r = ratio(forScreenHeightPoints: screen.frame.height)
        return CGSize(
            width: (baseWidth * r).rounded(),
            height: (baseHeight * r).rounded()
        )
    }

    /// Frame that places the scaled launcher horizontally centered, with its
    /// search bar (the window's top edge) a little above the screen's vertical
    /// center, so results grow downward from just above center - Spotlight-style,
    /// consistent on any display size or orientation. Clamped to stay fully
    /// within the visible area so it never runs off a short display.
    static func spotlightFrame(on screen: NSScreen) -> NSRect {
        let size = size(for: screen)
        let visible = screen.visibleFrame
        let x = visible.midX - size.width / 2
        // middle + height/2 + lift = window top; center the panel, then lift it.
        let windowTop = visible.midY + size.height / 2 + spotlightLift
        let y = min(max(windowTop - size.height, visible.minY), visible.maxY - size.height)
        return NSRect(x: x.rounded(), y: y.rounded(), width: size.width, height: size.height)
    }
}
