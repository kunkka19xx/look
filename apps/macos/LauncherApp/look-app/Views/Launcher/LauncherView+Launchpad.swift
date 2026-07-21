import Foundation

/// Empty-state launchpad wiring: loads the shared tile catalog once, exposes
/// whether the launchpad is currently on screen, and routes Command-mnemonic
/// key presses to it. Rendering lives in `EmptyStateLaunchpadView`; the mock
/// interactive state lives in `LaunchpadController`.
extension LauncherView {
    /// Decodes the shared launchpad layout once and wires the controller's
    /// banner sink. Idempotent, so it is safe to call from `onAppear`.
    func configureLaunchpadIfNeeded() {
        if launchpadTiles.isEmpty {
            launchpadTiles = EngineBridge.shared.launchpadLayout()
            launchpadController.configure(tiles: launchpadTiles)
        }
        // The controller cannot reach the view's banner directly; forward it.
        launchpadController.onBanner = { message in
            showBanner(message, style: .info, duration: 1.4)
        }
    }

    /// True while the empty-state launchpad is the visible content (empty query,
    /// not in command mode / settings / help). Gates the Command-mnemonic keys so
    /// they only fire when the strip is actually shown.
    var isLaunchpadActive: Bool {
        hidesResultsForEmptyQuery && !launchpadTiles.isEmpty
    }

    /// Routes a Command-mnemonic character to the launchpad. Returns true when a
    /// tile handled it (so the key monitor swallows the event).
    func handleLaunchpadMnemonic(_ character: Character) -> Bool {
        guard isLaunchpadActive else { return false }
        return launchpadController.handleMnemonic(character)
    }

    /// Cancels a pending Restart / Shut Down confirm on Escape. Returns true when
    /// a prompt was dismissed, so the monitor stops the key from also hiding the
    /// launcher.
    func cancelLaunchpadConfirmIfNeeded() -> Bool {
        guard isLaunchpadActive else { return false }
        return launchpadController.cancelConfirm()
    }
}
