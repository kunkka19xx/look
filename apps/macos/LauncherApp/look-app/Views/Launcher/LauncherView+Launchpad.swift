import Foundation

/// Empty-state launchpad wiring: loads the shared tile catalog once, exposes
/// whether the launchpad is currently on screen, and routes Command-mnemonic
/// key presses to it. Rendering lives in `EmptyStateLaunchpadView`; the mock
/// interactive state lives in `LaunchpadController`.
extension LauncherView {
    /// Decodes the shared launchpad layout once and wires the controller's
    /// banner sink. Idempotent, so it is safe to call from `onAppear`. Skipped
    /// entirely while Settings → Appearance → Super Actions is off.
    func configureLaunchpadIfNeeded() {
        guard themeStore.settings.superActionsEnabled else { return }
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
        themeStore.settings.superActionsEnabled && hidesResultsForEmptyQuery && !launchpadTiles.isEmpty
    }

    /// Builds and refreshes the launchpad when the Super Actions setting is
    /// switched on after the view already appeared, so the strip shows without a
    /// relaunch.
    func launchpadSettingChanged(enabled: Bool) {
        guard enabled else { return }
        configureLaunchpadIfNeeded()
        Task { await launchpadController.refreshStates() }
        Task { await launchpadController.refreshWeather() }
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
