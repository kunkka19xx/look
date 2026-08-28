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
            // Once per process, not per open: this branch is the first load, so
            // a broken drawing says so when the launcher first appears without
            // nagging on every summon after that.
            showLaunchpadWarnings(EngineBridge.shared.launchpadWarnings())
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
    /// Returns what is wrong with the drawing rather than showing it, so the
    /// caller folds it into one banner. `reloadConfig` warns that posting a
    /// second one "would replace any config warnings reported here before the
    /// user could read them", and a broken launchpad is exactly the case where
    /// both have something to say.
    @discardableResult
    func reloadLaunchpad() -> [String] {
        guard themeStore.settings.superActionsEnabled else { return [] }

        let reloaded = EngineBridge.shared.launchpadLayout()
        // Most reloads are about something else entirely, and the drawing is
        // usually untouched. Comparing first keeps those from re-reading every
        // adapter for a grid that did not move.
        if reloaded != launchpadTiles {
            launchpadTiles = reloaded
            launchpadController.configure(tiles: reloaded)
            // A tile the drawing just added has never been read: its adapter
            // state is resolved on launcher open, so without this it would sit
            // on the placeholder until the window was closed and reopened.
            refreshLaunchpadState()
        }

        // Asked for even when nothing moved, because that is what a broken
        // drawing looks like: it falls back to the default, so the tiles are
        // unchanged and the warning is the only sign anything happened.
        return EngineBridge.shared.launchpadWarnings()
    }

    /// Raises a launchpad config problem in the window. Nothing when the file
    /// is fine, which is almost always.
    func showLaunchpadWarnings(_ warnings: [String]) {
        guard let first = warnings.first else { return }
        // The count and the first message: the rest are on stderr and behind
        // the banner's copy button, and a banner is not a log.
        let message = warnings.count == 1 ? first : "\(first) (+\(warnings.count - 1) more)"
        showBanner(
            message,
            style: .warning,
            copyText: warnings.joined(separator: "\n"),
            duration: 5.0
        )
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
