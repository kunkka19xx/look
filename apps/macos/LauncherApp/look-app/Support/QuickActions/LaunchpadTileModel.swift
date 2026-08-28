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

/// How a tile is dressed - fonts, padding, glyph treatment - not how wide it
/// is. Width and height are `colSpan`/`rowSpan`, resolved by the core.
///
/// This used to carry a natural `columns`/`rows` that placement fell back to,
/// which meant a tile's footprint had two possible sources and the odd one out
/// (Weather: small, but two rows tall) needed an override to say so.
enum LaunchpadTileSize: String, Decodable {
    case l
    case m
    case s
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
    /// Where the tile sits, resolved by the core: zero-based column and row of
    /// its top-left cell, and how many cells it covers. Absolute, so this view
    /// layer places by offset rather than reconstructing an arrangement.
    let col: Int
    let row: Int
    let colSpan: Int
    let rowSpan: Int
    /// On/off captions for toggle tiles (e.g. Theme's Dark/Light). Nil for
    /// non-toggle tiles, which fall back to a generic On/Off.
    let onLabel: String?
    let offLabel: String?

    var id: String { actionId }

    /// Column span, clamped to at least 1 so a malformed catalog value can
    /// never produce a zero/negative tile width. No longer falls back to the
    /// tile size: the core resolves width, and `size` is presentation only.
    var columnSpan: Int { max(1, colSpan) }

    /// Row span, clamped for the same reason as `columnSpan`.
    var rowSpanCount: Int { max(1, rowSpan) }

    private enum CodingKeys: String, CodingKey {
        case actionId, title, size, role, mnemonic, col, row, colSpan, rowSpan, onLabel, offLabel
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        actionId = try container.decode(String.self, forKey: .actionId)
        title = try container.decode(String.self, forKey: .title)
        size = try container.decode(LaunchpadTileSize.self, forKey: .size)
        role = try container.decode(LaunchpadTileRole.self, forKey: .role)
        col = try container.decode(Int.self, forKey: .col)
        row = try container.decode(Int.self, forKey: .row)
        colSpan = try container.decode(Int.self, forKey: .colSpan)
        rowSpan = try container.decode(Int.self, forKey: .rowSpan)
        onLabel = try container.decodeIfPresent(String.self, forKey: .onLabel)
        offLabel = try container.decodeIfPresent(String.self, forKey: .offLabel)
        // serde serializes a `char` as a single-character string; take its first
        // character (nil when absent or empty).
        mnemonic = try container.decodeIfPresent(String.self, forKey: .mnemonic)?.first
    }
}
