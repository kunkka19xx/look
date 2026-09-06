import Foundation

/// Empty-state launchpad wiring: loads the shared tile catalog once, exposes
/// whether the launchpad is currently on screen, and routes Command-mnemonic
/// key presses to it. Rendering lives in `EmptyStateLaunchpadView`; the mock
/// interactive state lives in `LaunchpadController`.
extension LauncherView {
    /// The tiles alone, for everything that places or activates one.
    var launchpadTiles: [LaunchpadTileModel] { launchpadLayout.tiles }

    /// Decodes the shared launchpad layout once and wires the controller's
    /// banner sink. Idempotent, so it is safe to call from `onAppear`. Skipped
    /// entirely while Settings → Appearance → Super Actions is off.
    func configureLaunchpadIfNeeded() {
        guard themeStore.settings.superActionsEnabled else { return }
        if launchpadTiles.isEmpty {
            launchpadLayout = EngineBridge.shared.launchpadLayout()
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

    /// Re-reads the layout, ignoring the "already loaded" guard above:
    /// arranging tiles is an edit-and-look loop, and without this every edit
    /// would appear to do nothing until the app restarted.
    ///
    /// Returns the warnings rather than showing them, so the caller folds them
    /// into one banner - a second banner would replace the config warnings
    /// before the user could read them.
    @discardableResult
    func reloadLaunchpad() -> [String] {
        guard themeStore.settings.superActionsEnabled else { return [] }

        let reloaded = EngineBridge.shared.launchpadLayout()
        // Most reloads are about something else entirely, and the drawing is
        // usually untouched. Comparing first keeps those from re-reading every
        // adapter for a grid that did not move.
        if reloaded != launchpadLayout {
            launchpadLayout = reloaded
            launchpadController.configure(tiles: reloaded.tiles)
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
        // Off the main thread; the strip draws from the cache meanwhile.
        Task { await launchpadController.refreshCustomValues() }
    }

    /// Builds and refreshes the launchpad when the Super Actions setting is
    /// switched on after the view already appeared, so the strip shows without a
    /// relaunch.
    func launchpadSettingChanged(enabled: Bool) {
        guard enabled else { return }
        configureLaunchpadIfNeeded()
        refreshLaunchpadState()
    }

    /// Writes an arrangement the user dragged into place back to the drawing,
    /// then reads it again. Shown at once and confirmed by the file: the strip
    /// is what `~/.look/super-actions.toml` says, and a drag is one more way
    /// of editing that file.
    ///
    /// Saves run one after another, in drop order, and only the newest drop
    /// reads the file back. Two drops closer together than the disk is slow -
    /// a home on a network share, a scanner hooking every write - would
    /// otherwise race: the older save could land last, and its reload would
    /// put that arrangement over the newer one without a word.
    func launchpadArranged(_ tiles: [LaunchpadTileModel]) {
        typealias Const = AppConstants.Launcher.Launchpad
        let grid = LaunchpadGrid(
            tiles: tiles, declared: launchpadLayout.shape, rowHeight: Const.rowHeight, gap: Const.gap)
        let shape = LaunchpadGrid.Shape(columns: grid.columns, rows: grid.rows)

        launchpadLayout = LaunchpadLayout(tiles: tiles, shape: launchpadLayout.shape)
        // Mnemonics resolve through the controller's own copy of the layout.
        launchpadController.configure(tiles: tiles)

        launchpadSaveGeneration &+= 1
        let generation = launchpadSaveGeneration
        let previous = launchpadSave
        launchpadSave = Task {
            await previous?.value
            let failure = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.saveLaunchpadLayout(tiles, shape: shape)
            }.value
            if let failure {
                showBanner(failure, style: .error, duration: 4.0)
            }
            // A newer drop is waiting its turn, and its reload is the one that
            // counts: reading the file back now would show this arrangement
            // over the one the user last dropped, until that one lands.
            guard generation == launchpadSaveGeneration else { return }
            // The file is the truth either way: a refused write re-reads the
            // drawing the tiles came from and they spring back; a good one
            // re-reads exactly what was written, which changes nothing.
            reloadLaunchpad()
        }
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
