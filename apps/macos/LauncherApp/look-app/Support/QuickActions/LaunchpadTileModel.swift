import Foundation

/// Swift mirror of the shared `look_qactions::LaunchpadTile`. Decoded from the
/// FFI JSON with `.convertFromSnakeCase`, so `action_id` -> `actionId`. The
/// order, sizes, and mnemonics come from the shared core catalog; this app only
/// renders them and resolves live state natively.

/// The shared `action_id` strings from `look_qactions::action_id`, mirrored here
/// so the launchpad UI never hard-codes bare literals.
enum LaunchpadActionID {
    static let lSlot = "lslot"
    static let bluetooth = "bluetooth"
    static let wifi = "wifi"
    static let theme = "theme"
    static let keepAwake = "keepawake"
    static let screensaver = "screensaver"
    static let mic = "mic"
    static let restart = "restart"
    static let shutdown = "shutdown"
    static let battery = "battery"
    static let weather = "weather"
    static let nowPlaying = "nowplaying"
}

/// A launchpad tile's footprint in the 6-column bento grid.
enum LaunchpadTileSize: String, Decodable {
    case l
    case m
    case s

    /// Natural column span for this size before any per-tile `span` override.
    var columns: Int {
        switch self {
        case .l, .m: return 2
        case .s: return 1
        }
    }

    /// Row span for this size.
    var rows: Int {
        switch self {
        case .l: return 2
        case .m, .s: return 1
        }
    }
}

/// How a launchpad tile is drawn (see the Rust `TileRole`).
enum LaunchpadTileRole: String, Decodable {
    case toggle
    case info
    case action
    case media
    case weather
    case slot
}

/// A single launchpad tile, decoded from the shared catalog.
struct LaunchpadTileModel: Decodable, Identifiable, Equatable {
    let actionId: String
    let title: String
    let size: LaunchpadTileSize
    let role: LaunchpadTileRole
    /// The mnemonic character (triggered with Command). Nil for the L slot and
    /// Battery. Decoded from a one-character JSON string.
    let mnemonic: Character?
    /// Column-span override (Now Playing spans 3). Nil uses `size.columns`.
    let span: Int?
    /// Row-span override (Weather stands 2 rows tall). Nil uses `size.rows`.
    let rowSpan: Int?
    /// On/off captions for toggle tiles (e.g. Theme's Dark/Light). Nil for
    /// non-toggle tiles, which fall back to a generic On/Off.
    let onLabel: String?
    let offLabel: String?

    var id: String { actionId }

    /// Effective column span: the per-tile override when present, else the
    /// natural width of the tile size. Clamped to at least 1 so a malformed
    /// catalog value can never produce a zero/negative tile width.
    var columnSpan: Int { max(1, span ?? size.columns) }

    /// Effective row span: the per-tile override when present, else the natural
    /// height of the tile size. Clamped to at least 1 for the same reason as
    /// `columnSpan`.
    var rowSpanCount: Int { max(1, rowSpan ?? size.rows) }

    private enum CodingKeys: String, CodingKey {
        case actionId, title, size, role, mnemonic, span, rowSpan, onLabel, offLabel
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        actionId = try container.decode(String.self, forKey: .actionId)
        title = try container.decode(String.self, forKey: .title)
        size = try container.decode(LaunchpadTileSize.self, forKey: .size)
        role = try container.decode(LaunchpadTileRole.self, forKey: .role)
        span = try container.decodeIfPresent(Int.self, forKey: .span)
        rowSpan = try container.decodeIfPresent(Int.self, forKey: .rowSpan)
        onLabel = try container.decodeIfPresent(String.self, forKey: .onLabel)
        offLabel = try container.decodeIfPresent(String.self, forKey: .offLabel)
        // serde serializes a `char` as a single-character string; take its first
        // character (nil when absent or empty).
        mnemonic = try container.decodeIfPresent(String.self, forKey: .mnemonic)?.first
    }
}
