import AppKit
import OSLog
import SwiftUI

struct WindowConfigurator: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            configureWindow(from: view, force: true)
            if let window = view.window {
                suppressTitlebarHairline(in: window)
            }
        }
        return view
    }

    // updateNSView is called every time SwiftUI re-evaluates the view tree
    // containing WindowConfigurator. Re-running configureWindow on every
    // update was causing visible flicker during drag: setting styleMask,
    // isOpaque, layer.cornerRadius, masksToBounds, etc. on a moving window
    // forces CALayer recomposition mid-drag. Window properties here are all
    // constant, so we only run that block once and just re-assert the things
    // AppKit resets on its own (window buttons, titlebar separator).
    func updateNSView(_ nsView: NSView, context: Context) {
        DispatchQueue.main.async {
            configureWindow(from: nsView, force: false)
            guard let window = nsView.window else { return }
            // AppKit re-shows the standard window buttons whenever the window
            // is resized / restyled, so re-hide them on every update. Cheap
            // (sets three NSView.isHidden flags).
            hideStandardButtons(in: window)
        }
    }

    private func hideStandardButtons(in window: NSWindow) {
        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
    }

    // macOS Sequoia draws a 1px titlebar separator on the first paint even
    // though `titlebarSeparatorStyle = .none` is set in configureWindow.
    // The style is only honored after a *real* frame change - re-assigning
    // `.none` alone does nothing. Reproduce that frame change here: once the
    // window has painted, apply a 1px round-trip resize. It's imperceptible
    // but forces AppKit to re-evaluate the separator and drop the hairline.
    // As a fallback, also hide AppKit's private separator subview if it
    // re-materializes.
    //
    // ONE-SHOT per window. The setFrame grow/restore must NOT run on every
    // updateNSView (typing, hover, running-app notifications all trigger it),
    // or the window would visibly resize twice per state change - flicker, and
    // the same transient deactivation that WindowAutoScale suppresses only for
    // its own resizes. Runs once, on first show, from makeNSView.
    private func suppressTitlebarHairline(in window: NSWindow) {
        let id = ObjectIdentifier(window)
        guard !hairlineSuppressedWindowIDs.contains(id) else { return }
        hairlineSuppressedWindowIDs.insert(id)

        window.titlebarSeparatorStyle = .none
        hideTitlebarSeparatorSubviews(in: window)

        // The frame change must actually persist for one runloop pass before
        // being reverted - growing and restoring in the same tick gets
        // coalesced by AppKit into a net no-op, so `.none` never re-applies.
        // Grow now, restore on the next tick.
        let original = window.frame
        var grown = original
        grown.size.height += 1
        window.setFrame(grown, display: true)

        DispatchQueue.main.async {
            window.setFrame(original, display: true)
            window.titlebarSeparatorStyle = .none
            hideTitlebarSeparatorSubviews(in: window)
        }
    }

    // Fallback: AppKit's titlebar separator is a private view nested in the
    // theme frame (NSTitlebarContainerView → NSTitlebarView → separator).
    // Walk the whole hierarchy and hide anything that looks like it.
    private func hideTitlebarSeparatorSubviews(in window: NSWindow) {
        guard let root = window.contentView?.superview else { return }
        func walk(_ view: NSView) {
            let name = String(describing: type(of: view))
            if name.contains("TitlebarSeparator") || name.contains("NSTitlebarDecorationView") {
                view.isHidden = true
            }
            view.subviews.forEach(walk)
        }
        walk(root)
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
        // macOS Sequoia honors `.none` only after the first real frame resize;
        // suppressTitlebarHairline() forces that on first show. See its comment.
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
        // Position is preserved (top-left anchor) - only size changes,
        // since the macOS launcher is user-draggable.
        NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeScreenNotification,
            object: window,
            queue: .main
        ) { note in
            guard let w = note.object as? NSWindow else { return }
            Logger(subsystem: "noah-code.Look", category: "window-resize")
                .debug("didChangeScreenNotification fired - scheduling resize")
            // The observer is registered with `queue: .main`, so this already
            // runs on the main actor - assert it to reach the isolated method.
            MainActor.assumeIsolated {
                WindowAutoScale.scheduleResize(for: w)
            }
        }
    }
}

// One-shot guard so configureWindow runs exactly once per NSWindow.
// Only ever read/written on the main actor (configureWindow is invoked
// from SwiftUI's main-actor view-update path), but the global is
// otherwise unprotected - declare its isolation explicitly for Swift 6.
@MainActor private var configuredWindowIDs: Set<ObjectIdentifier> = []

// Windows whose first-show titlebar-hairline workaround has already run, so the
// 1px setFrame round-trip happens exactly once per window and never on routine
// updateNSView passes. Main-actor only, like configuredWindowIDs above.
@MainActor private var hairlineSuppressedWindowIDs: Set<ObjectIdentifier> = []
