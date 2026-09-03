import Foundation
import Observation

/// Holds the interactive state for the empty-state launchpad and routes tile
/// activations (clicks and Command-mnemonics) to it.
///
/// Tiles backed by a native `SystemControl` (via `ActionAdapterRegistry`) read
/// and drive real system state; tiles whose adapter has not been written yet
/// fall back to local mock state, so each control goes live the moment its
/// adapter is registered, without touching this controller. Restart / Shut Down
/// open an inline confirm before running.
@MainActor
@Observable
final class LaunchpadController {
    /// Mock on/off state for toggle tiles that have no native adapter yet, the
    /// framework's graceful fallback. Currently empty: every launchpad toggle is
    /// adapter-backed. An adapter-backed tile ignores this and reads
    /// `systemStates` instead.
    private(set) var toggles: [String: Bool] = [:]

    /// Live state for adapter-backed tiles, keyed by `action_id`, refreshed from
    /// each control's `state()` read.
    private(set) var systemStates: [String: ActionState] = [:]

    /// The Battery adapter's resolved info fields (currently just "charging"),
    /// refreshed alongside `systemStates`. Info tiles are the only role that
    /// reads `info()`; only Battery has one today.
    private(set) var batteryInfo: [String: InfoValue] = [:]

    /// Whether the battery is actively charging, for the launchpad tile's icon.
    var batteryCharging: Bool {
        if case .text(let text) = batteryInfo["charging"] { return text == "charging" }
        return false
    }

    /// Mic mute state (true = muted); rendered amber when muted. Backed by the
    /// Mic adapter: muted means its state read as `.off` (input volume 0).
    var micMuted: Bool { systemStates[LaunchpadActionID.mic] == .off }

    /// Latest resolved weather for the Weather tile, or nil until the first
    /// successful fetch (the tile shows a placeholder meanwhile).
    private(set) var weather: WeatherSnapshot?
    /// Last value each user tile's command printed, keyed by the name in the
    /// drawing. Filled by the value cache.
    private(set) var customValues: [String: LaunchpadTileValue] = [:]

    private let weatherService = WeatherService.shared

    /// The system-wide now-playing track (any app), or nil when nothing plays.
    private(set) var nowPlaying: NowPlayingSnapshot?

    /// The play state a just-issued command should produce, held until a read
    /// agrees. Commands apply asynchronously, so without this the tile flips
    /// three times per press: optimistic, stale read, then the truth.
    @ObservationIgnored private var expectedIsPlaying: Bool?
    @ObservationIgnored private var expectationDeadline = Date.distantPast
    @ObservationIgnored private var nowPlayingReconcile: Task<Void, Never>?

    /// Stops an earlier read resuming last over a newer one, as `stateGeneration` does.
    @ObservationIgnored private var nowPlayingGeneration: UInt64 = 0

    private static let nowPlayingSettleNanos: UInt64 = 300_000_000
    /// Cap on holding the expected state, so a command that never lands cannot
    /// freeze the tile.
    private static let nowPlayingSettleTimeout: TimeInterval = 2.5

    /// The destructive tile currently awaiting an inline confirm, or nil.
    private(set) var pendingConfirmActionID: String?

    /// The tile layout, needed to resolve a pressed mnemonic to a tile.
    private var tiles: [LaunchpadTileModel] = []

    /// Called to surface a short banner for action feedback. Set by the view.
    var onBanner: ((String) -> Void)?

    func configure(tiles: [LaunchpadTileModel]) {
        self.tiles = tiles
        // So a tile with a value shows it on this frame, not a placeholder.
        if tiles.contains(role: .custom) {
            customValues = EngineBridge.shared.launchpadTileValues()
        }
    }

    /// Re-runs stale tile commands. Detached: the launchpad is already on
    /// screen from the cache, and a late value belongs to a tile still there.
    func refreshCustomValues() async {
        // Cheap is not free: without this every open reads and parses
        // super-actions.toml to discover there is nothing to run.
        guard tiles.contains(role: .custom) else { return }

        let outcome = await Task.detached(priority: .utility) {
            EngineBridge.shared.refreshLaunchpadTiles()
        }.value

        if outcome.refreshed > 0 {
            customValues = await Task.detached(priority: .utility) {
                EngineBridge.shared.launchpadTileValues()
            }.value
        }
        for failure in outcome.errors {
            onBanner?(failure)
        }
    }

    /// Bumped by every write to `systemStates`. Each `state()` read suspends, so
    /// two refreshes (or a refresh and a toggle) interleave on the main actor and
    /// the slower one would otherwise land last and revert the tiles.
    @ObservationIgnored private var stateGeneration: UInt64 = 0

    /// Reads the current state of every adapter-backed tile. Called on launcher
    /// open so the strip reflects reality; tiles without an adapter are skipped
    /// and keep their mock fallback. A snapshot superseded while it was being
    /// gathered is dropped rather than applied.
    func refreshStates() async {
        stateGeneration &+= 1
        let generation = stateGeneration
        var resolved: [String: ActionState] = [:]
        var resolvedBatteryInfo: [String: InfoValue] = [:]
        for tile in tiles {
            guard let adapter = ActionAdapterRegistry.adapter(for: tile.actionId) else { continue }
            resolved[tile.actionId] = await adapter.state()
            if tile.actionId == LaunchpadActionID.battery {
                resolvedBatteryInfo = await adapter.info(keys: ["charging"])
            }
        }
        guard generation == stateGeneration else { return }
        systemStates = resolved
        batteryInfo = resolvedBatteryInfo
    }

    /// Resolves the Weather tile's value. Cheap to call on every launcher open:
    /// the service serves a fresh cache without touching the network and only
    /// refetches once the reading goes stale. Assigned unconditionally: the
    /// service already falls back to a compatible cache on failure, so a nil
    /// result means there is no usable reading and the tile should clear rather
    /// than keep a stale value (e.g. in the wrong unit after a region change).
    /// Skipped when the drawing placed no weather tile: cheap is not free.
    func refreshWeather() async {
        guard tiles.contains(role: .weather) else { return }
        weather = await weatherService.currentWeather()
    }

    /// Reads the current now-playing track. While a command is still settling,
    /// fresh metadata is adopted but the expected play state is kept.
    func refreshNowPlaying() async {
        nowPlayingGeneration &+= 1
        let generation = nowPlayingGeneration
        let fresh = await SystemNowPlaying.shared.current()
        guard generation == nowPlayingGeneration else { return }

        guard let expected = expectedIsPlaying, Date() < expectationDeadline else {
            expectedIsPlaying = nil
            nowPlaying = fresh
            return
        }
        if fresh?.isPlaying == expected {
            expectedIsPlaying = nil
            nowPlaying = fresh
            return
        }
        // Reads come back empty when the player moves mid-read. Blanking here
        // would strip the path the next press needs.
        guard let fresh else { return }
        nowPlaying = NowPlayingSnapshot(
            title: fresh.title,
            artist: fresh.artist,
            app: fresh.app,
            isPlaying: expected,
            playerPath: fresh.playerPath
        )
    }

    /// Toggles play/pause on the player the tile is showing. Flips optimistically
    /// so the button responds without waiting for the read.
    func nowPlayingToggle() {
        guard let current = nowPlaying else { return }
        let target = !current.isPlaying
        nowPlaying = NowPlayingSnapshot(
            title: current.title,
            artist: current.artist,
            app: current.app,
            isPlaying: target,
            playerPath: current.playerPath
        )
        expectedIsPlaying = target
        expectationDeadline = Date().addingTimeInterval(Self.nowPlayingSettleTimeout)
        issueNowPlayingCommand(.togglePlayPause)
    }

    /// Skips the current system media to the next track.
    func nowPlayingNext() { issueNowPlayingCommand(.nextTrack) }

    /// Returns the current system media to the previous track.
    func nowPlayingPrevious() { issueNowPlayingCommand(.previousTrack) }

    /// Sends a command to the player on the tile, then re-reads until it lands.
    /// Targeting that exact player is what stops "pause, then play" from
    /// resuming Pomodoro music instead of the browser.
    private func issueNowPlayingCommand(_ command: SystemNowPlaying.Command) {
        let target = nowPlaying?.playerPath
        nowPlayingReconcile?.cancel()
        nowPlayingReconcile = Task {
            guard await SystemNowPlaying.shared.send(command, to: target) else {
                // Undelivered, so no change is coming: drop the optimistic flip
                // now rather than polling to the deadline.
                expectedIsPlaying = nil
                await refreshNowPlaying()
                return
            }
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.nowPlayingSettleNanos)
                guard !Task.isCancelled else { return }
                await refreshNowPlaying()
                guard expectedIsPlaying != nil else { return }
            }
        }
    }

    func isOn(_ actionID: String) -> Bool {
        if ActionAdapterRegistry.adapter(for: actionID) != nil {
            return systemStates[actionID] == .on
        }
        return toggles[actionID] ?? false
    }

    /// The display value for a read-only info tile (e.g. Battery), taken from its
    /// adapter's `.value` state. Nil while unavailable or not yet read, so the
    /// tile can show a placeholder.
    func displayValue(for actionID: String) -> String? {
        if case .value(let text) = systemStates[actionID] { return text }
        return nil
    }

    /// What a user tile's `value` command last printed. Nil before the first
    /// run, which is the normal first state - the same as Battery's.
    func customValue(for actionID: String) -> LaunchpadTileValue? {
        customValues[actionID]
    }

    /// True only once the adapter has reported the tile as genuinely unavailable
    /// (e.g. Battery on a desktop Mac), as opposed to not-yet-read. Lets the
    /// Battery tile fall back to showing uptime instead of a dead "--".
    func isUnavailable(_ actionID: String) -> Bool {
        if case .unavailable = systemStates[actionID] { return true }
        return false
    }

    /// Activates a tile. Destructive tiles gate on an inline confirm first;
    /// everything else routes to its native adapter when one exists, and falls
    /// back to mock state otherwise.
    func activate(_ tile: LaunchpadTileModel) {
        // Any activation other than confirming the pending tile clears a stale
        // confirm, so the prompt never lingers on an unrelated press.
        if pendingConfirmActionID != nil, pendingConfirmActionID != tile.actionId {
            pendingConfirmActionID = nil
        }

        // One gate, whatever the tile is: a first press on anything that asks
        // arms it, a second fires. Restart and Shut Down used to be named here
        // by id, which meant the same two ids were listed in three places.
        if tile.confirm?.isEmpty == false, pendingConfirmActionID != tile.actionId {
            pendingConfirmActionID = tile.actionId
            return
        }
        pendingConfirmActionID = nil
        dispatch(tile)
    }

    /// What a tile does once anything it wanted to ask has been answered.
    private func dispatch(_ tile: LaunchpadTileModel) {
        // Now Playing reflects and controls the system-wide media (any app), not a
        // SystemControl adapter, so the play/pause key routes a transport command.
        if tile.actionId == LaunchpadActionID.nowPlaying {
            nowPlayingToggle()
            return
        }

        // No adapter: the core holds the command and runs it by name.
        if tile.role == .custom {
            guard tile.pressable else { return }
            pressCustom(tile)
            return
        }

        if let adapter = ActionAdapterRegistry.adapter(for: tile.actionId) {
            perform(intent(for: tile), on: adapter, actionID: tile.actionId)
        } else {
            activateMock(tile)
        }
    }

    /// Fires the armed tile, then clears the prompt.
    func confirmPending() {
        guard let actionID = pendingConfirmActionID,
              let tile = tiles.first(where: { $0.actionId == actionID })
        else { return }
        pendingConfirmActionID = nil
        dispatch(tile)
    }

    /// Cancels the inline confirm. Returns true if a prompt was actually
    /// dismissed, so the caller (Esc handler) knows to swallow the key.
    @discardableResult
    func cancelConfirm() -> Bool {
        guard pendingConfirmActionID != nil else { return false }
        pendingConfirmActionID = nil
        return true
    }

    /// Routes a Command-mnemonic key press to its tile. Returns true when a tile
    /// matched and was activated (so the key monitor swallows the event).
    func handleMnemonic(_ character: Character) -> Bool {
        let lowered = Character(character.lowercased())
        guard let tile = tiles.first(where: { tile in
            guard let mnemonic = tile.mnemonic else { return false }
            return Character(mnemonic.lowercased()) == lowered
        }) else { return false }
        activate(tile)
        return true
    }

    // MARK: - Adapter routing

    /// The intent a tile maps to. A tile that carries on/off captions is a
    /// toggle (flip it: Bluetooth, Wi-Fi, Theme, Keep Awake, Mic mute); a
    /// label-less action is a fire-once button (Screensaver). Restart / Shut Down
    /// take the confirm path and Now Playing its own transport, so neither
    /// reaches here.
    private func intent(for tile: LaunchpadTileModel) -> ActionIntent {
        if tile.role == .toggle { return .toggle }
        return tile.offLabel == nil ? .run : .toggle
    }

    /// Applies an intent to an adapter off the main run loop, reports the
    /// outcome, then re-reads the control's state so the tile reflects reality.
    /// Supersedes any refresh still in flight, whose snapshot predates the change
    /// and would flip the tile back.
    private func perform(_ intent: ActionIntent, on adapter: any SystemControl, actionID: String) {
        Task {
            report(await adapter.apply(intent))
            stateGeneration &+= 1
            let generation = stateGeneration
            let state = await adapter.state()
            guard generation == stateGeneration else { return }
            systemStates[actionID] = state
            systemStates[actionID] = await adapter.state()
        }
    }

    private func report(_ outcome: ActionOutcome) {
        switch outcome {
        case .ok(let banner):
            if let banner { onBanner?(banner) }
        case .failed(let message), .needsPermission(let message):
            onBanner?(message)
        }
    }

    /// Runs a press off the main thread, then re-reads: a press usually
    /// changes the thing the tile reports.
    private func pressCustom(_ tile: LaunchpadTileModel) {
        let name = tile.actionId
        Task {
            let failure = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.pressLaunchpadTile(name)
            }.value
            if let failure {
                onBanner?(failure)
                return
            }
            // Read straight from the cache the press just wrote. Going through
            // `refreshCustomValues` would find nothing stale - the core read
            // this tile as part of the press - and so re-read nothing.
            customValues = await Task.detached(priority: .utility) {
                EngineBridge.shared.launchpadTileValues()
            }.value
        }
    }

/// Fallback behavior for a toggle tile without a native adapter: flip local
    /// state. Unused while every launchpad toggle is adapter-backed; kept as the
    /// framework's graceful path for a future tile added ahead of its adapter.
    private func activateMock(_ tile: LaunchpadTileModel) {
        if tile.role == .toggle {
            toggles[tile.actionId] = !(toggles[tile.actionId] ?? false)
        }
    }
}
