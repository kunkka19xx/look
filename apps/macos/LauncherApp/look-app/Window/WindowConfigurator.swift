import AppKit
import OSLog
import SwiftUI

struct WindowConfigurator: NSViewRepresentable {
    let placement: RunningAppsPlacement

    init(placement: RunningAppsPlacement = .none) {
        self.placement = placement
    }

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
    // all constant, so we only run that block once. The running-apps
    // placement is the one thing that needs to react — resize the window
    // when the user picks a new placement.
    func updateNSView(_ nsView: NSView, context: Context) {
        let placement = self.placement
        DispatchQueue.main.async {
            configureWindow(from: nsView, force: false)
            guard let window = nsView.window else { return }
            // AppKit re-shows the standard window buttons whenever the
            // window is resized / restyled, so re-hide them on every
            // update. Cheap (sets three NSView.isHidden flags) and
            // prevents the title-bar buttons from flashing back when the
            // user changes running-apps placement in Settings.
            hideStandardButtons(in: window)
            let last = lastAppliedPlacement[ObjectIdentifier(window)]
            if last != placement {
                lastAppliedPlacement[ObjectIdentifier(window)] = placement
                WindowAutoScale.resizeKeepingTopLeft(window, placement: placement)
                hideStandardButtons(in: window)
            }
        }
    }

    private func hideStandardButtons(in window: NSWindow) {
        window.standardWindowButton(.closeButton)?.isHidden = true
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
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
        // Known issue (macOS Sequoia): AppKit draws a 1px hairline at the
        // top of the content area on the first paint even with `.none` here
        // — the setting is honored only after the first real frame resize.
        // Toggling running-apps placement (Settings → Running Apps) clears
        // it for the session. Parked in docs/tasks.md "Parked: known issues".
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
            window.setFrame(WindowAutoScale.centeredFrame(on: screen, placement: placement), display: true)
        }
        lastAppliedPlacement[ObjectIdentifier(window)] = placement

        // Re-apply scaling when the window crosses to a different display.
        // Position is preserved (top-left anchor) — only size changes,
        // since the macOS launcher is user-draggable.
        let currentPlacement = placement
        NotificationCenter.default.addObserver(
            forName: NSWindow.didChangeScreenNotification,
            object: window,
            queue: .main
        ) { note in
            guard let w = note.object as? NSWindow else { return }
            let p = lastAppliedPlacement[ObjectIdentifier(w)] ?? currentPlacement
            Logger(subsystem: "noah-code.Look", category: "window-resize")
                .debug("didChangeScreenNotification fired — placement=\(p.rawValue, privacy: .public), scheduling resize")
            WindowAutoScale.scheduleResize(for: w, placement: p)
        }
    }
}

@MainActor private var lastAppliedPlacement: [ObjectIdentifier: RunningAppsPlacement] = [:]

// One-shot guard so configureWindow runs exactly once per NSWindow.
// Only ever read/written on the main actor (configureWindow is invoked
// from SwiftUI's main-actor view-update path), but the global is
// otherwise unprotected — declare its isolation explicitly for Swift 6.
@MainActor private var configuredWindowIDs: Set<ObjectIdentifier> = []
