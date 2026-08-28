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

    /// Re-reads the layout, ignoring the "already loaded" guard above.
    ///
    /// The tiles are decoded once per process, which was right while the grid
    /// was a compile-time constant. It is not any more: ~/.look/launchpad.toml
    /// decides the arrangement, and arranging tiles is an edit-and-look loop.
    /// Without this, every edit would appear to do nothing until the app was
    /// restarted, which reads as the feature being broken rather than as the
    /// layout being cached.
    func reloadLaunchpad() {
        guard themeStore.settings.superActionsEnabled else { return }

        let reloaded = EngineBridge.shared.launchpadLayout()
        // Most reloads are about something else entirely, and the drawing is
        // usually untouched. Comparing first keeps those from re-reading every
        // adapter for a grid that did not move.
        guard reloaded != launchpadTiles else { return }

        launchpadTiles = reloaded
        launchpadController.configure(tiles: reloaded)
        // A tile the drawing just added has never been read: its adapter state
        // is resolved on launcher open, so without this it would sit on the
        // placeholder until the window was closed and reopened.
        refreshLaunchpadState()
    }

    /// True while the empty-state launchpad is the visible content (empty query,
    /// not in command mode / settings / help). Gates the Command-mnemonic keys so
    /// they only fire when the strip is actually shown.
    var isLaunchpadActive: Bool {
        themeStore.settings.superActionsEnabled && hidesResultsForEmptyQuery
            && !launchpadTiles.isEmpty && !isAIMode
    }

    /// Re-reads adapter-backed tiles and weather. Called on every open: the
    /// window is only ordered out, so `onAppear` fires once per process.
    func refreshLaunchpadState() {
        guard themeStore.settings.superActionsEnabled else { return }
        Task { await launchpadController.refreshStates() }
        Task { await launchpadController.refreshWeather() }
    }

    /// Builds and refreshes the launchpad when the Super Actions setting is
    /// switched on after the view already appeared, so the strip shows without a
    /// relaunch.
    func launchpadSettingChanged(enabled: Bool) {
        guard enabled else { return }
        configureLaunchpadIfNeeded()
        refreshLaunchpadState()
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
