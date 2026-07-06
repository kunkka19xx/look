import AppKit
import Foundation
import OSLog

private let windowAutoScaleLog = Logger(subsystem: "noah-code.Look", category: "window-resize")

/// Screen-based auto-scale for the launcher window.
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
/// Recentering on every show is intentionally omitted: the macOS
/// launcher is draggable (isMovableByWindowBackground), so users
/// place it where they want. We size and center once, on first
/// window attach.
enum WindowAutoScale {
    // Base size matches the Linux/Windows build (apps/linows/src-tauri):
    // 860×580 logical, landscape - list pane + preview pane side by side.
    static let baseWidth: CGFloat = 860
    static let baseHeight: CGFloat = 600

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

    /// Frame that centers the scaled launcher within the screen's
    /// visibleFrame (so it doesn't overlap the menu bar or Dock).
    static func centeredFrame(on screen: NSScreen) -> NSRect {
        let size = size(for: screen)
        let visible = screen.visibleFrame
        let x = visible.origin.x + (visible.width - size.width) / 2
        let y = visible.origin.y + (visible.height - size.height) / 2
        return NSRect(x: x.rounded(), y: y.rounded(), width: size.width, height: size.height)
    }

    /// Resize the window to match the current screen's scale, keeping the
    /// existing position anchor (top-left). Used when the user drags the
    /// window to a different display - we adjust the scale but never the
    /// position, since the macOS launcher is user-draggable.
    ///
    /// Clamps the result inside the screen's visibleFrame so we never
    /// leave the window partially off-screen (e.g. tucked behind a notch
    /// or menu bar after a screen with different geometry).
    @MainActor
    static func resizeKeepingTopLeft(_ window: NSWindow) {
        guard let screen = window.screen else {
            windowAutoScaleLog.debug("resize: window.screen is nil, skipping")
            return
        }
        let newSize = size(for: screen)
        let currentFrame = window.frame
        let visible = screen.visibleFrame
        windowAutoScaleLog.debug("resize start: screen=\(screen.frame.debugDescription, privacy: .public) visible=\(visible.debugDescription, privacy: .public) current=\(currentFrame.debugDescription, privacy: .public) newSize=\(newSize.debugDescription, privacy: .public) isVisible=\(window.isVisible, privacy: .public)")

        if currentFrame.size == newSize {
            windowAutoScaleLog.debug("resize: size unchanged, skipping")
            return
        }

        // AppKit's frame origin is bottom-left; preserve the visual top by
        // shifting origin.y when height changes.
        var newOrigin = NSPoint(
            x: currentFrame.origin.x,
            y: currentFrame.origin.y + currentFrame.height - newSize.height
        )

        let maxX = visible.maxX - newSize.width
        let minX = visible.minX
        let maxY = visible.maxY - newSize.height
        let minY = visible.minY
        newOrigin.x = min(max(newOrigin.x, minX), maxX)
        newOrigin.y = min(max(newOrigin.y, minY), maxY)

        let newFrame = NSRect(origin: newOrigin, size: newSize)
        windowAutoScaleLog.debug("resize -> newFrame=\(newFrame.debugDescription, privacy: .public) (clamp bounds x=\(minX, privacy: .public)..\(maxX, privacy: .public), y=\(minY, privacy: .public)..\(maxY, privacy: .public))")
        if newFrame != currentFrame {
            Self.lastProgrammaticResizeAt = Date()
            window.setFrame(newFrame, display: true, animate: false)
            windowAutoScaleLog.debug("resize done: applied=\(window.frame.debugDescription, privacy: .public) isVisible=\(window.isVisible, privacy: .public)")
        }
    }

    /// Defer a resize until the user is no longer holding the mouse
    /// (i.e. the drag has ended). Calling `setFrame` mid-drag fights
    /// AppKit's mouse tracking and leaves the window in a broken state.
    /// Each call supersedes any pending resize for the same window so
    /// rapid screen crossings collapse to a single resize when the
    /// drag finishes.
    @MainActor private static var pendingResizeTokens: [ObjectIdentifier: UUID] = [:]

    /// Timestamp of the most recent programmatic `setFrame` from this module.
    /// `didResignActiveNotification` can fire shortly after a screen-drag
    /// resize because the size change briefly knocks the launcher off
    /// "frontmost" status. Surface a "did we just resize?" check so the
    /// launcher's auto-hide-on-resign handler can ignore those events.
    @MainActor private static var lastProgrammaticResizeAt: Date?

    static let resizeSettleWindow: TimeInterval = 1.0

    @MainActor
    static func didProgrammaticallyResizeRecently() -> Bool {
        guard let t = lastProgrammaticResizeAt else { return false }
        return Date().timeIntervalSince(t) < resizeSettleWindow
    }

    @MainActor
    static func scheduleResize(for window: NSWindow) {
        let id = ObjectIdentifier(window)
        let token = UUID()
        pendingResizeTokens[id] = token
        // Start the suppression window NOW (at notification time) - even
        // if the resize turns out to be a no-op (e.g., same scale across
        // screens), the screen change itself can briefly drop the
        // launcher's frontmost status and trigger the auto-hide.
        Self.lastProgrammaticResizeAt = Date()

        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 50_000_000)
            while pendingResizeTokens[id] == token {
                // Mouse button still down → user is still dragging.
                // Resize only after the button is released so we don't
                // fight AppKit's mouse tracking mid-drag.
                if NSEvent.pressedMouseButtons & 1 != 0 {
                    try? await Task.sleep(nanoseconds: 80_000_000)
                    continue
                }
                pendingResizeTokens[id] = nil
                resizeKeepingTopLeft(window)
                return
            }
        }
    }
}
