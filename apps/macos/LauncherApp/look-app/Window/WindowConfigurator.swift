import AppKit
import SwiftUI

struct WindowConfigurator: NSViewRepresentable {
    /// Observed, not read off the shared store: only a change SwiftUI sees
    /// re-runs updateNSView to restyle the live window.
    @ObservedObject var themeStore: ThemeStore

    private var cornerRadius: CGFloat {
        themeStore.panelRadius
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            configureWindow(from: view, force: true)
        }
        return view
    }

    // Re-running configureWindow on every update flickers: restyling a live
    // window forces CALayer recomposition. Its properties are constant, so it
    // runs once and only the corner radius is re-asserted here.
    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            configureWindow(from: nsView, force: false)
            guard let window = nsView.window else { return }
            applyCornerRadius(cornerRadius, in: window)
        }
    }

    /// Rounds the window itself. The panel's SwiftUI clip cannot: the backdrop
    /// is an AppKit blur the window server composites past it.
    private func applyCornerRadius(_ radius: CGFloat, in window: NSWindow) {
        for view in [window.contentView?.superview, window.contentView].compactMap({ $0 }) {
            view.wantsLayer = true
            guard let layer = view.layer else { continue }
            if layer.cornerRadius != radius {
                layer.cornerRadius = radius
            }
            if !layer.masksToBounds {
                layer.masksToBounds = true
            }
        }
    }

    private func configureWindow(from view: NSView, force: Bool) {
        guard let window = view.window else { return }
        if !force, configuredWindowIDs.contains(ObjectIdentifier(window)) { return }
        configuredWindowIDs.insert(ObjectIdentifier(window))

        window.isOpaque = false
        window.backgroundColor = .clear
        // The launcher is not user-movable: it opens at a fixed
        // position on the active (cursor) screen every show, so movement would
        // only let its position drift out of sync. See toggleWindowVisibility.
        window.isMovableByWindowBackground = false
        window.isMovable = false
        window.hasShadow = false
        window.collectionBehavior.insert(.moveToActiveSpace)
        window.collectionBehavior.insert(.fullScreenAuxiliary)
        // Float above other apps so dragging the launcher onto a screen
        // where another app has a window in front does not bury it.
        // Matches the Linux/Windows build's set_always_on_top(true).
        window.level = .floating

        applyCornerRadius(cornerRadius, in: window)

        // Initial placement; toggleWindowVisibility re-places it on the active
        // screen every show, so this only seeds a sane frame before first show.
        if let screen = window.screen ?? NSScreen.main {
            window.setFrame(WindowAutoScale.spotlightFrame(on: screen), display: true)
        }
    }
}

// One-shot guard so configureWindow runs exactly once per NSWindow.
// Only ever read/written on the main actor (configureWindow is invoked
// from SwiftUI's main-actor view-update path), but the global is
// otherwise unprotected - declare its isolation explicitly for Swift 6.
@MainActor private var configuredWindowIDs: Set<ObjectIdentifier> = []
