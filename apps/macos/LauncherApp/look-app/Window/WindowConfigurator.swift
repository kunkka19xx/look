import AppKit
import SwiftUI

struct WindowConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            configureWindow(from: view, force: true)
        }
        return view
    }

    // updateNSView is called every time SwiftUI re-evaluates the view tree
    // containing WindowConfigurator. Re-running configureWindow on every
    // update was causing visible flicker during drag: setting styleMask,
    // isOpaque, layer.cornerRadius, masksToBounds, etc. on a moving window
    // forces CALayer recomposition mid-drag. Window properties here are
    // all constant, so we only configure once (when the window first
    // attaches) — subsequent updates are a no-op.
    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            configureWindow(from: nsView, force: false)
        }
    }

    private func configureWindow(from view: NSView, force: Bool) {
        guard let window = view.window else { return }
        if !force, configuredWindowIDs.contains(ObjectIdentifier(window)) { return }
        configuredWindowIDs.insert(ObjectIdentifier(window))

        window.styleMask.insert(.titled)
        window.styleMask.remove(.closable)
        window.styleMask.remove(.miniaturizable)
        window.styleMask.remove(.resizable)
        window.styleMask.remove(.borderless)
        window.styleMask.insert(.fullSizeContentView)
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.toolbar = nil
        window.isOpaque = false
        window.backgroundColor = .clear
        window.isMovableByWindowBackground = true
        window.hasShadow = false
        window.setContentBorderThickness(0, for: .maxY)
        window.collectionBehavior.insert(.moveToActiveSpace)
        window.collectionBehavior.insert(.fullScreenAuxiliary)
        // Float above other apps so dragging the launcher onto a screen
        // where another app has a window in front does not bury it.
        // Matches the Linux/Windows build's set_always_on_top(true).
        window.level = .floating
        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true

        let cornerRadius = AppConstants.Launcher.windowCornerRadius
        if let frameView = window.contentView?.superview {
            frameView.wantsLayer = true
            frameView.layer?.cornerRadius = cornerRadius
            frameView.layer?.masksToBounds = true
        }

        if let contentView = window.contentView {
            contentView.wantsLayer = true
            contentView.layer?.cornerRadius = cornerRadius
            contentView.layer?.masksToBounds = true
        }

        if let screen = window.screen ?? NSScreen.main {
            window.setFrame(WindowAutoScale.centeredFrame(on: screen), display: true)
        }

        // Re-apply scaling when the window crosses to a different display.
        // Position is preserved (top-left anchor) — only size changes,
        // since the macOS launcher is user-draggable.
        NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeScreenNotification,
            object: window,
            queue: .main
        ) { note in
            guard let w = note.object as? NSWindow else { return }
            WindowAutoScale.scheduleResize(for: w)
        }
    }
}

// One-shot guard so configureWindow runs exactly once per NSWindow.
// Only ever read/written on the main actor (configureWindow is invoked
// from SwiftUI's main-actor view-update path), but the global is
// otherwise unprotected — declare its isolation explicitly for Swift 6.
@MainActor private var configuredWindowIDs: Set<ObjectIdentifier> = []
