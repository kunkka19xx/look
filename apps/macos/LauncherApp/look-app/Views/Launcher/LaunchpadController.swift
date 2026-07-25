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

    /// Mic mute state (true = muted); rendered amber when muted. Backed by the
    /// Mic adapter: muted means its state read as `.off` (input volume 0).
    var micMuted: Bool { systemStates[LaunchpadActionID.mic] == .off }

    /// Latest resolved weather for the Weather tile, or nil until the first
    /// successful fetch (the tile shows a placeholder meanwhile).
    private(set) var weather: WeatherSnapshot?

    private let weatherService = WeatherService.shared

    /// The system-wide now-playing track (any app), or nil when nothing plays.
    private(set) var nowPlaying: NowPlayingSnapshot?

    /// When the last transport command was issued. A background poll ignores a
    /// read for this long afterward so it cannot clobber the optimistic update
    /// before the command has settled; the forced reconcile applies the truth.
    @ObservationIgnored private var lastNowPlayingCommandAt = Date.distantPast
    @ObservationIgnored private var nowPlayingReconcile: Task<Void, Never>?

    /// Delay before re-reading after a command (commands apply asynchronously).
    private static let nowPlayingSettleNanos: UInt64 = 350_000_000
    /// A poll read is skipped when a command was issued within this window.
    private static let nowPlayingCommandGuard: TimeInterval = 0.4

    /// The destructive tile currently awaiting an inline confirm, or nil.
    private(set) var pendingConfirmActionID: String?

    /// The tile layout, needed to resolve a pressed mnemonic to a tile.
    private var tiles: [LaunchpadTileModel] = []

    /// Called to surface a short banner for action feedback. Set by the view.
    var onBanner: ((String) -> Void)?

    func configure(tiles: [LaunchpadTileModel]) {
        self.tiles = tiles
    }

    /// Reads the current state of every adapter-backed tile. Called on launcher
    /// open so the strip reflects reality; tiles without an adapter are skipped
    /// and keep their mock fallback.
    func refreshStates() async {
        var resolved: [String: ActionState] = [:]
        for tile in tiles {
            guard let adapter = ActionAdapterRegistry.adapter(for: tile.actionId) else { continue }
            resolved[tile.actionId] = await adapter.state()
        }
        systemStates = resolved
    }

    /// Resolves the Weather tile's value. Cheap to call on every launcher open:
    /// the service serves a fresh cache without touching the network and only
    /// refetches once the reading goes stale. Assigned unconditionally: the
    /// service already falls back to a compatible cache on failure, so a nil
    /// result means there is no usable reading and the tile should clear rather
    /// than keep a stale value (e.g. in the wrong unit after a region change).
    func refreshWeather() async {
        weather = await weatherService.currentWeather()
    }

    /// Reads the current system-wide now-playing track. `force` bypasses the
    /// post-command guard and is used by the reconcile after a transport command;
    /// the periodic poll passes `force: false` so it never overrides a fresh
    /// optimistic update mid-flight.
    func refreshNowPlaying(force: Bool = false) async {
        if !force, Date().timeIntervalSince(lastNowPlayingCommandAt) < Self.nowPlayingCommandGuard {
            return
        }
        nowPlaying = await SystemNowPlaying.shared.current()
    }

    /// Toggles play/pause on whatever app owns system now-playing.
    func nowPlayingToggle() {
        // Optimistic: flip immediately so the button responds without waiting for
        // the read; the reconcile confirms the real state shortly after.
        if let current = nowPlaying {
            nowPlaying = NowPlayingSnapshot(
                title: current.title,
                artist: current.artist,
                app: current.app,
                isPlaying: !current.isPlaying
            )
        }
        issueNowPlayingCommand(.togglePlayPause)
    }

    /// Skips the current system media to the next track.
    func nowPlayingNext() { issueNowPlayingCommand(.nextTrack) }

    /// Returns the current system media to the previous track.
    func nowPlayingPrevious() { issueNowPlayingCommand(.previousTrack) }

    /// Sends a transport command, then re-reads once the change has settled.
    private func issueNowPlayingCommand(_ command: SystemNowPlaying.Command) {
        lastNowPlayingCommandAt = Date()
        SystemNowPlaying.shared.send(command)
        nowPlayingReconcile?.cancel()
        nowPlayingReconcile = Task {
            try? await Task.sleep(nanoseconds: Self.nowPlayingSettleNanos)
            guard !Task.isCancelled else { return }
            await refreshNowPlaying(force: true)
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

        if tile.actionId == LaunchpadActionID.restart || tile.actionId == LaunchpadActionID.shutdown {
            if pendingConfirmActionID == tile.actionId {
                confirmPending()
            } else {
                pendingConfirmActionID = tile.actionId
            }
            return
        }

        // Now Playing reflects and controls the system-wide media (any app), not a
        // SystemControl adapter, so the play/pause key routes a transport command.
        if tile.actionId == LaunchpadActionID.nowPlaying {
            nowPlayingToggle()
            return
        }

        if let adapter = ActionAdapterRegistry.adapter(for: tile.actionId) {
            perform(intent(for: tile), on: adapter, actionID: tile.actionId)
        } else {
            activateMock(tile)
        }
    }

    /// Fires the pending destructive action, then clears the prompt. Routes to a
    /// native adapter when one is registered; otherwise reports a demo banner.
    func confirmPending() {
        guard let actionID = pendingConfirmActionID else { return }
        pendingConfirmActionID = nil
        if let adapter = ActionAdapterRegistry.adapter(for: actionID) {
            perform(.run, on: adapter, actionID: actionID)
        } else {
            let label = actionID == LaunchpadActionID.restart ? "Restart" : "Shut Down"
            onBanner?("\(label) (demo)")
        }
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
    private func perform(_ intent: ActionIntent, on adapter: any SystemControl, actionID: String) {
        Task {
            report(await adapter.apply(intent))
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

    /// Fallback behavior for a toggle tile without a native adapter: flip local
    /// state. Unused while every launchpad toggle is adapter-backed; kept as the
    /// framework's graceful path for a future tile added ahead of its adapter.
    private func activateMock(_ tile: LaunchpadTileModel) {
        if tile.role == .toggle {
            toggles[tile.actionId] = !(toggles[tile.actionId] ?? false)
        }
    }
}
