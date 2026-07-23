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
    /// Mock on/off state for toggle tiles that have no native adapter yet. An
    /// adapter-backed tile ignores this and reads `systemStates` instead.
    private(set) var toggles: [String: Bool] = [
        LaunchpadActionID.focus: false,
        LaunchpadActionID.saver: false,
    ]

    /// Live state for adapter-backed tiles, keyed by `action_id`, refreshed from
    /// each control's `state()` read.
    private(set) var systemStates: [String: ActionState] = [:]

    /// Mic mute state (true = muted); rendered amber when muted. Mock until a Mic
    /// adapter lands.
    private(set) var micMuted = false

    /// Now Playing transport state. Mock until a Now Playing adapter lands.
    private(set) var isPlaying = false

    /// Latest resolved weather for the Weather tile, or nil until the first
    /// successful fetch (the tile shows a placeholder meanwhile).
    private(set) var weather: WeatherSnapshot?

    private let weatherService = WeatherService.shared

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

    /// The intent a tile's role maps to. Toggles and Mic flip; Now Playing runs
    /// its transport. Restart / Shut Down are handled via the confirm path.
    private func intent(for tile: LaunchpadTileModel) -> ActionIntent {
        switch tile.role {
        case .media: return .run
        default: return .toggle
        }
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

    /// Fallback behavior for tiles without a native adapter: flip local state.
    private func activateMock(_ tile: LaunchpadTileModel) {
        switch tile.actionId {
        case LaunchpadActionID.mic:
            micMuted.toggle()
            onBanner?(micMuted ? "Mic muted" : "Mic on")
        case LaunchpadActionID.nowPlaying:
            isPlaying.toggle()
        default:
            if tile.role == .toggle {
                toggles[tile.actionId] = !(toggles[tile.actionId] ?? false)
            }
        }
    }
}
