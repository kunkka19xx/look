import Foundation
import Observation

/// Holds the interactive state for the empty-state launchpad and routes tile
/// activations (clicks and Command-mnemonics) to it.
///
/// This is the layout-only pass: toggles and the mic/now-playing controls flip
/// local mock state, and Restart / Shut Down open an inline confirm that fires a
/// banner instead of a real power action. Wiring each tile to a live
/// `SystemControl` adapter (Wi-Fi, Theme, Focus, Saver, Mic, ...) lands in
/// follow-up PRs; only this controller and the native reads change then.
@MainActor
@Observable
final class LaunchpadController {
    /// On/off state for the stateful toggle tiles, keyed by `action_id`.
    /// Seeded with representative mock values for the layout pass.
    private(set) var toggles: [String: Bool] = [
        LaunchpadActionID.bluetooth: false,
        LaunchpadActionID.wifi: true,
        LaunchpadActionID.theme: true,
        LaunchpadActionID.focus: false,
        LaunchpadActionID.saver: false,
    ]

    /// Mic mute state (true = muted); rendered amber when muted.
    private(set) var micMuted = false

    /// Now Playing transport state (mock).
    private(set) var isPlaying = false

    /// Latest resolved weather for the Weather tile, or nil until the first
    /// successful fetch (the tile shows a placeholder meanwhile).
    private(set) var weather: WeatherSnapshot?

    private let weatherService = WeatherService.shared

    /// The destructive tile currently awaiting an inline confirm, or nil.
    private(set) var pendingConfirmActionID: String?

    /// The tile layout, needed to resolve a pressed mnemonic to a tile.
    private var tiles: [LaunchpadTileModel] = []

    /// Called to surface a short banner for mock actions. Set by the view.
    var onBanner: ((String) -> Void)?

    func configure(tiles: [LaunchpadTileModel]) {
        self.tiles = tiles
    }

    /// Resolves the Weather tile's value. Cheap to call on every launcher open:
    /// the service serves a fresh cache without touching the network and only
    /// refetches once the reading goes stale.
    func refreshWeather() async {
        if let snapshot = await weatherService.currentWeather() {
            weather = snapshot
        }
    }

    func isOn(_ actionID: String) -> Bool {
        toggles[actionID] ?? false
    }

    /// Activates a tile: toggles flip, Mic mutes, Now Playing plays/pauses, and
    /// the destructive tiles open (or confirm) the inline confirm prompt.
    func activate(_ tile: LaunchpadTileModel) {
        // Any activation other than confirming the pending tile clears a stale
        // confirm, so the prompt never lingers on an unrelated press.
        if pendingConfirmActionID != nil, pendingConfirmActionID != tile.actionId {
            pendingConfirmActionID = nil
        }

        switch tile.actionId {
        case LaunchpadActionID.restart, LaunchpadActionID.shutdown:
            if pendingConfirmActionID == tile.actionId {
                confirmPending()
            } else {
                pendingConfirmActionID = tile.actionId
            }
        case LaunchpadActionID.mic:
            micMuted.toggle()
            onBanner?(micMuted ? "Mic muted" : "Mic on")
        case LaunchpadActionID.nowPlaying:
            isPlaying.toggle()
        default:
            if tile.role == .toggle {
                toggles[tile.actionId] = !isOn(tile.actionId)
            }
        }
    }

    /// Fires the pending destructive action (mock) and clears the prompt.
    func confirmPending() {
        guard let actionID = pendingConfirmActionID else { return }
        let label = actionID == LaunchpadActionID.restart ? "Restart" : "Shut Down"
        onBanner?("\(label) (demo)")
        pendingConfirmActionID = nil
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
}
