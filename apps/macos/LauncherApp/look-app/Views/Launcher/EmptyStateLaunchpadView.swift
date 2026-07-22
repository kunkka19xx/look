import SwiftUI

/// The empty-state launchpad: a 6-column bento of L/M/S tiles shown below the
/// search bar when the query is empty (see "Empty State Spec"). Tile order,
/// sizes, labels, and mnemonics come from the shared `look_qactions` catalog;
/// this view lays them out and (in this pass) renders mock state, except the L
/// slot, which reads the live Todo / Pomo stores.
struct EmptyStateLaunchpadView: View {
    let tiles: [LaunchpadTileModel]
    var controller: LaunchpadController
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    private var byID: [String: LaunchpadTileModel] {
        Dictionary(tiles.map { ($0.actionId, $0) }, uniquingKeysWith: { first, _ in first })
    }

    var body: some View {
        GeometryReader { geo in
            let cell = cellWidth(total: geo.size.width)
            layout(cell: cell)
        }
        .frame(height: totalHeight)
        .padding(.top, Const.outerTopPadding)
    }

    // MARK: Geometry

    private func cellWidth(total: CGFloat) -> CGFloat {
        let gaps = Const.gap * CGFloat(Const.columns - 1)
        return max(0, (total - gaps) / CGFloat(Const.columns))
    }

    private func width(columns: Int, cell: CGFloat) -> CGFloat {
        CGFloat(columns) * cell + Const.gap * CGFloat(columns - 1)
    }

    private func height(rows: Int) -> CGFloat {
        CGFloat(rows) * Const.rowHeight + Const.gap * CGFloat(rows - 1)
    }

    private var totalHeight: CGFloat { height(rows: 3) }

    // MARK: Layout
    //
    // The spec's fixed order tiles a 6-column grid as three rows:
    //   row 0/1 cols 0-1: L slot (2x2)   | cols 2-5: BT, Wi-Fi, Battery(2)
    //                                     |           Theme(2), Focus, Saver
    //   row 2 cols 0-5: Mic, Restart, Shut Down, Now Playing(3)
    // SwiftUI has no cell spanning, so the arrangement is composed explicitly
    // from the known order rather than auto-packed.

    @ViewBuilder
    private func layout(cell: CGFloat) -> some View {
        VStack(spacing: Const.gap) {
            HStack(alignment: .top, spacing: Const.gap) {
                slot(cell: cell)
                    .frame(width: width(columns: 2, cell: cell), height: height(rows: 2))

                VStack(spacing: Const.gap) {
                    HStack(spacing: Const.gap) {
                        tileView(LaunchpadActionID.bluetooth, cell: cell)
                        tileView(LaunchpadActionID.wifi, cell: cell)
                        tileView(LaunchpadActionID.battery, cell: cell)
                    }
                    HStack(spacing: Const.gap) {
                        tileView(LaunchpadActionID.theme, cell: cell)
                        tileView(LaunchpadActionID.focus, cell: cell)
                        tileView(LaunchpadActionID.saver, cell: cell)
                    }
                }
            }

            HStack(spacing: Const.gap) {
                tileView(LaunchpadActionID.mic, cell: cell)
                tileView(LaunchpadActionID.restart, cell: cell)
                tileView(LaunchpadActionID.shutdown, cell: cell)
                tileView(LaunchpadActionID.nowPlaying, cell: cell)
            }
        }
    }

    @ViewBuilder
    private func slot(cell: CGFloat) -> some View {
        LaunchpadLSlotView(themeStore: themeStore)
    }

    /// Renders the tile for `actionID` at its natural size, sourcing labels and
    /// the mnemonic from the decoded catalog model.
    @ViewBuilder
    private func tileView(_ actionID: String, cell: CGFloat) -> some View {
        if let model = byID[actionID] {
            let w = width(columns: model.columnSpan, cell: cell)
            let h = height(rows: model.size.rows)
            tileContent(model)
                .frame(width: w, height: h)
        }
    }

    @ViewBuilder
    private func tileContent(_ model: LaunchpadTileModel) -> some View {
        switch model.role {
        case .toggle:
            LaunchpadToggleTile(
                model: model,
                isOn: controller.isOn(model.actionId),
                themeStore: themeStore
            ) { controller.activate(model) }
        case .info:
            LaunchpadInfoTile(model: model, value: Const.mockBatteryValue, themeStore: themeStore)
        case .action:
            LaunchpadActionTile(
                model: model,
                themeStore: themeStore,
                micMuted: model.actionId == LaunchpadActionID.mic ? controller.micMuted : false,
                confirming: controller.pendingConfirmActionID == model.actionId
            ) { controller.activate(model) }
        case .media:
            LaunchpadMediaTile(
                model: model,
                isPlaying: controller.isPlaying,
                themeStore: themeStore
            ) { controller.activate(model) }
        case .slot:
            EmptyView()
        }
    }
}

// MARK: - Mnemonic helper

/// Builds a label with one character tinted in the theme's warning color (the
/// spec's "yellow"), matching `⌘<char>`. Falls back to the plain label when the
/// mnemonic character does not occur in it.
private func mnemonicText(
    _ label: String,
    mnemonic: Character?,
    font: Font,
    base: Color,
    highlight: Color
) -> Text {
    guard let mnemonic,
          let range = label.range(
            of: String(mnemonic),
            options: [.caseInsensitive]
          )
    else {
        return Text(label).font(font).foregroundColor(base)
    }
    let before = String(label[label.startIndex..<range.lowerBound])
    let match = String(label[range])
    let after = String(label[range.upperBound...])
    return Text(before).font(font).foregroundColor(base)
        + Text(match).font(font).foregroundColor(highlight)
        + Text(after).font(font).foregroundColor(base)
}

// MARK: - Tiles

/// A stateful on/off tile (Bluetooth, Wi-Fi, Theme, Focus, Saver). On uses the
/// accent color plus a subtle accent border.
private struct LaunchpadToggleTile: View {
    let model: LaunchpadTileModel
    let isOn: Bool
    var themeStore: ThemeStore
    var onTap: () -> Void

    private typealias Const = AppConstants.Launcher.Launchpad

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 8) {
                Image(systemName: iconName)
                    .font(.system(size: 18, weight: .medium))
                    .foregroundColor(isOn ? themeStore.accentColor() : themeStore.mutedTextColor())
                VStack(alignment: .leading, spacing: 1) {
                    mnemonicText(
                        model.title,
                        mnemonic: model.mnemonic,
                        font: themeStore.uiFont(size: Const.smallLabelFontSize, weight: .semibold),
                        base: isOn ? themeStore.fontColor() : themeStore.secondaryTextColor(),
                        highlight: themeStore.warningColor()
                    )
                    .lineLimit(1)
                    Text(stateLabel)
                        .font(themeStore.uiFont(size: Const.captionFontSize - 1))
                        .foregroundColor(isOn ? themeStore.accentColor() : themeStore.mutedTextColor())
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .background(
                frostedTile(
                    themeStore: themeStore,
                    tint: isOn ? themeStore.accentColor() : nil,
                    tintOpacity: isOn ? launchpadOnTintOpacity : 0
                )
            )
            .overlay(tileBorder(isOn: isOn, themeStore: themeStore))
        }
        .buttonStyle(.plain)
    }

    private var stateLabel: String {
        if isOn { return model.onLabel ?? "On" }
        return model.offLabel ?? "Off"
    }

    private var iconName: String {
        switch model.actionId {
        case LaunchpadActionID.bluetooth: return "antenna.radiowaves.left.and.right"
        case LaunchpadActionID.wifi: return "wifi"
        case LaunchpadActionID.theme: return isOn ? "moon.fill" : "sun.max.fill"
        case LaunchpadActionID.focus: return "moon.circle.fill"
        case LaunchpadActionID.saver: return "bolt.fill"
        default: return "circle"
        }
    }
}

/// A read-only info tile (Battery): label plus a live value.
private struct LaunchpadInfoTile: View {
    let model: LaunchpadTileModel
    let value: String
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "battery.100")
                .font(.system(size: 18, weight: .medium))
                .foregroundColor(themeStore.accentColor())
            VStack(alignment: .leading, spacing: 1) {
                Text(model.title.uppercased())
                    .font(themeStore.uiFont(size: Const.captionFontSize - 1, weight: .medium))
                    .foregroundColor(themeStore.mutedTextColor())
                Text(value)
                    .font(themeStore.uiFont(size: Const.valueFontSize - 6, weight: .bold))
                    .foregroundColor(themeStore.fontColor())
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(frostedTile(themeStore: themeStore))
        .overlay(tileBorder(isOn: false, themeStore: themeStore))
    }
}

/// A compact system-action tile (Mic, Restart, Shut Down). Mic turns amber when
/// muted; Restart / Shut Down use the caution color and show an inline confirm.
private struct LaunchpadActionTile: View {
    let model: LaunchpadTileModel
    var themeStore: ThemeStore
    let micMuted: Bool
    let confirming: Bool
    var onTap: () -> Void

    private typealias Const = AppConstants.Launcher.Launchpad

    private var isDanger: Bool {
        model.actionId == LaunchpadActionID.restart || model.actionId == LaunchpadActionID.shutdown
    }

    private var tint: Color {
        if confirming { return themeStore.dangerColor() }
        if micMuted { return themeStore.warningColor() }
        if isDanger { return themeStore.dangerColor() }
        return themeStore.secondaryTextColor()
    }

    var body: some View {
        Button(action: onTap) {
            VStack(spacing: 6) {
                Image(systemName: iconName)
                    .font(.system(size: 18, weight: .medium))
                    .foregroundColor(tint)
                mnemonicText(
                    confirming ? "Confirm?" : model.title,
                    mnemonic: confirming ? nil : model.mnemonic,
                    font: themeStore.uiFont(size: Const.smallLabelFontSize, weight: .semibold),
                    base: confirming ? themeStore.dangerColor() : themeStore.secondaryTextColor(),
                    highlight: themeStore.warningColor()
                )
                .lineLimit(1)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(
                frostedTile(
                    themeStore: themeStore,
                    tint: (confirming || micMuted) ? tint : nil,
                    tintOpacity: (confirming || micMuted) ? launchpadAlertTintOpacity : 0
                )
            )
            .overlay(
                RoundedRectangle(cornerRadius: Const.cornerRadius, style: .continuous)
                    .strokeBorder(
                        (confirming || micMuted)
                            ? tint.opacity(launchpadAlertBorderOpacity)
                            : themeStore.dividerColor().opacity(launchpadRestingBorderOpacity),
                        lineWidth: launchpadBorderWidth
                    )
            )
        }
        .buttonStyle(.plain)
    }

    private var iconName: String {
        switch model.actionId {
        case LaunchpadActionID.mic: return micMuted ? "mic.slash.fill" : "mic.fill"
        case LaunchpadActionID.restart: return "arrow.clockwise"
        case LaunchpadActionID.shutdown: return "power"
        default: return "circle"
        }
    }
}

/// The Now Playing tile: track name plus a Pause/Play control (⌘P).
private struct LaunchpadMediaTile: View {
    let model: LaunchpadTileModel
    let isPlaying: Bool
    var themeStore: ThemeStore
    var onToggle: () -> Void

    private typealias Const = AppConstants.Launcher.Launchpad

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "music.note")
                .font(.system(size: 18, weight: .medium))
                .foregroundColor(themeStore.accentColor())
            VStack(alignment: .leading, spacing: 1) {
                Text(model.title.uppercased())
                    .font(themeStore.uiFont(size: Const.captionFontSize - 1, weight: .medium))
                    .foregroundColor(themeStore.mutedTextColor())
                Text(trackTitle)
                    .font(themeStore.uiFont(size: Const.titleFontSize, weight: .bold))
                    .foregroundColor(isPlaying ? themeStore.fontColor() : themeStore.mutedTextColor())
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            Button(action: onToggle) {
                VStack(spacing: 2) {
                    Image(systemName: isPlaying ? "pause.fill" : "play.fill")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundColor(themeStore.fontColor())
                    mnemonicText(
                        isPlaying ? "Pause" : "Play",
                        mnemonic: model.mnemonic,
                        font: themeStore.uiFont(size: Const.captionFontSize - 1, weight: .semibold),
                        base: themeStore.secondaryTextColor(),
                        highlight: themeStore.warningColor()
                    )
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
                .background(themeStore.controlFillColor())
                .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(frostedTile(themeStore: themeStore))
        .overlay(tileBorder(isOn: false, themeStore: themeStore))
    }

    private var trackTitle: String {
        isPlaying ? "Kiasmos - Blurred" : AppConstants.Launcher.Launchpad.mockNowPlayingTitle
    }
}

// MARK: - Shared tile chrome

/// Scrim opacity layered over the blur, matching `LauncherView.tileBackground`
/// so launchpad tiles read with the same depth as the rest of the floating UI.
private let launchpadTileScrimOpacity = 0.30
private let launchpadOnTintOpacity = 0.22
/// Backs both the red confirm state and the amber muted-mic state, so the tile
/// takes its hue from `tint` rather than baking one in here.
private let launchpadAlertTintOpacity = 0.16

/// Tile borders. Every tile rests on the same neutral divider so the strip reads
/// as one surface; colour is reserved for a state the user just entered, an
/// on-state toggle or an armed confirm, never for a tile sitting idle.
private let launchpadBorderWidth: CGFloat = 1
private let launchpadRestingBorderOpacity = 0.4
private let launchpadOnBorderOpacity = 0.35
private let launchpadAlertBorderOpacity = 0.3

/// The frosted surface every launchpad tile sits on: the same
/// blur + dark-scrim + control-fill stack the app uses for its floating tiles
/// (`LauncherView.tileBackground(floats:)`), so opacity and material stay
/// unified. An optional tint (accent for on-state toggles, caution for a
/// confirming/muted action) layers on top.
func frostedTile(themeStore: ThemeStore, tint: Color? = nil, tintOpacity: Double = 0) -> some View {
    let radius = AppConstants.Launcher.Launchpad.cornerRadius
    return ZStack {
        VisualEffectBlur(material: themeStore.settings.blurMaterial.material)
        Color.black.opacity(launchpadTileScrimOpacity)
        themeStore.controlFillColor()
        if let tint {
            tint.opacity(tintOpacity)
        }
    }
    .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
}

private func tileBorder(isOn: Bool, themeStore: ThemeStore) -> some View {
    let radius = AppConstants.Launcher.Launchpad.cornerRadius
    return RoundedRectangle(cornerRadius: radius, style: .continuous)
        .strokeBorder(
            isOn
                ? themeStore.accentColor().opacity(launchpadOnBorderOpacity)
                : themeStore.dividerColor().opacity(launchpadRestingBorderOpacity),
            lineWidth: launchpadBorderWidth
        )
}
