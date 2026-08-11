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
    /// Changes each time the launcher opens, replaying the spawn cascade.
    var revealToken: UInt64 = 0

    private typealias Const = AppConstants.Launcher.Launchpad

    /// Reading-order position of each tile in the spawn cascade, so the grid
    /// settles in top-left to bottom-right rather than all at once.
    private enum RevealIndex {
        static let slot = 0
        static let bluetooth = 1
        static let wifi = 2
        static let battery = 3
        static let theme = 4
        static let keepAwake = 5
        static let screensaver = 6
        static let weather = 7
        static let mic = 8
        static let restart = 9
        static let shutdown = 10
        static let nowPlaying = 11
    }

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
        // Poll system now-playing while the launchpad is on screen, so external
        // changes (pausing in a browser) are reflected. Cancelled on disappear.
        .task {
            while !Task.isCancelled {
                await controller.refreshNowPlaying()
                try? await Task.sleep(for: .seconds(Const.nowPlayingPollSeconds))
            }
        }
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
    //   row 0/1 cols 0-1: L slot (2x2) | cols 2-4: BT, Wi-Fi, Battery
    //                                   |          Theme, Keep Awake, Screensaver
    //                                   | col 5:   Weather (1x2)
    //   row 2 cols 0-5: Mic, Restart, Shut Down, Now Playing(3)
    // SwiftUI has no cell spanning, so the arrangement is composed explicitly
    // from the known order rather than auto-packed.

    @ViewBuilder
    private func layout(cell: CGFloat) -> some View {
        VStack(spacing: Const.gap) {
            HStack(alignment: .top, spacing: Const.gap) {
                slot(cell: cell)
                    .frame(width: width(columns: 2, cell: cell), height: height(rows: 2))
                    .spawnReveal(index: RevealIndex.slot, token: revealToken)

                VStack(spacing: Const.gap) {
                    HStack(spacing: Const.gap) {
                        tileView(LaunchpadActionID.bluetooth, cell: cell, reveal: RevealIndex.bluetooth)
                        tileView(LaunchpadActionID.wifi, cell: cell, reveal: RevealIndex.wifi)
                        tileView(LaunchpadActionID.battery, cell: cell, reveal: RevealIndex.battery)
                    }
                    HStack(spacing: Const.gap) {
                        tileView(LaunchpadActionID.theme, cell: cell, reveal: RevealIndex.theme)
                        tileView(LaunchpadActionID.keepAwake, cell: cell, reveal: RevealIndex.keepAwake)
                        tileView(LaunchpadActionID.screensaver, cell: cell, reveal: RevealIndex.screensaver)
                    }
                }

                tileView(LaunchpadActionID.weather, cell: cell, reveal: RevealIndex.weather)
            }

            HStack(spacing: Const.gap) {
                tileView(LaunchpadActionID.mic, cell: cell, reveal: RevealIndex.mic)
                tileView(LaunchpadActionID.restart, cell: cell, reveal: RevealIndex.restart)
                tileView(LaunchpadActionID.shutdown, cell: cell, reveal: RevealIndex.shutdown)
                tileView(LaunchpadActionID.nowPlaying, cell: cell, reveal: RevealIndex.nowPlaying)
            }
        }
    }

    @ViewBuilder
    private func slot(cell: CGFloat) -> some View {
        LaunchpadLSlotView(themeStore: themeStore)
    }

    /// Renders the tile for `actionID` at its natural size, sourcing labels and
    /// the mnemonic from the decoded catalog model. `reveal` places it in the
    /// spawn cascade.
    @ViewBuilder
    private func tileView(_ actionID: String, cell: CGFloat, reveal: Int) -> some View {
        if let model = byID[actionID] {
            let w = width(columns: model.columnSpan, cell: cell)
            let h = height(rows: model.rowSpanCount)
            tileContent(model)
                .frame(width: w, height: h)
                .spawnReveal(index: reveal, token: revealToken)
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
            LaunchpadInfoTile(
                model: model,
                batteryValue: controller.displayValue(for: model.actionId),
                showsUptime: model.actionId == LaunchpadActionID.battery
                    && controller.isUnavailable(model.actionId),
                charging: controller.batteryCharging,
                themeStore: themeStore
            )
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
                snapshot: controller.nowPlaying,
                themeStore: themeStore,
                onToggle: { controller.activate(model) },
                onPrevious: { controller.nowPlayingPrevious() },
                onNext: { controller.nowPlayingNext() }
            )
        case .weather:
            LaunchpadWeatherTile(
                model: model,
                weather: controller.weather,
                themeStore: themeStore
            )
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

/// A stateful on/off tile (Bluetooth, Wi-Fi, Theme, Keep Awake). On uses the
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
                    // Theme and Keep Awake swap glyph on flip, so the icon
                    // crossfades; the others keep one glyph and just react.
                    .contentTransition(.symbolEffect(.replace))
                    .symbolEffect(.bounce, value: isOn)
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
        .buttonStyle(PressableSurfaceStyle())
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
        case LaunchpadActionID.keepAwake: return isOn ? "cup.and.saucer.fill" : "cup.and.saucer"
        default: return "circle"
        }
    }
}

/// A read-only info tile (Battery): label plus a live value. On a machine with
/// no battery (e.g. a Mac mini), it falls back to showing system uptime instead
/// of a dead placeholder.
private struct LaunchpadInfoTile: View {
    let model: LaunchpadTileModel
    /// The battery percent string (e.g. "85%"), or nil while unread / unavailable.
    let batteryValue: String?
    /// True when there is no battery, so the tile shows uptime instead.
    let showsUptime: Bool
    /// True while the battery is actively charging, so the tile shows the
    /// bolt variant of the battery icon.
    let charging: Bool
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    private var iconName: String {
        if showsUptime { return Const.uptimeIconName }
        return charging ? Const.batteryChargingIconName : Const.batteryIconName
    }

    private var label: String {
        (showsUptime ? Const.uptimeLabel : model.title).uppercased()
    }

    private var value: String {
        showsUptime ? SystemUptime.formattedShort() : (batteryValue ?? Const.infoPlaceholderValue)
    }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: iconName)
                .font(.system(size: 18, weight: .medium))
                .foregroundColor(themeStore.accentColor())
                // The battery glyph gains and loses its bolt as charging starts
                // and stops, and fills as the level changes.
                .contentTransition(.symbolEffect(.replace))
            VStack(alignment: .leading, spacing: 1) {
                Text(label)
                    .font(themeStore.uiFont(size: Const.captionFontSize - 1, weight: .medium))
                    .foregroundColor(themeStore.mutedTextColor())
                Text(value)
                    .font(themeStore.uiFont(size: Const.valueFontSize - 6, weight: .bold))
                    .foregroundColor(themeStore.fontColor())
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                    .contentTransition(.numericText())
                    .animation(Motion.Value.rollDigits, value: value)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(frostedTile(themeStore: themeStore))
        .overlay(tileBorder(isOn: false, themeStore: themeStore))
    }
}

/// A read-only weather tile: a tall single-column tile stacking a condition
/// icon, the temperature, the condition, and today's high/low and rain chance.
/// Shows a placeholder icon and value until the first reading resolves.
private struct LaunchpadWeatherTile: View {
    let model: LaunchpadTileModel
    let weather: WeatherSnapshot?
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    /// The caption under the temperature: the live condition once known, else
    /// the tile title while the placeholder shows.
    private var caption: String {
        (weather?.condition ?? model.title).uppercased()
    }

    var body: some View {
        VStack(spacing: 5) {
            Image(systemName: weather?.symbolName ?? "cloud.sun.fill")
                .font(.system(size: 20, weight: .medium))
                .foregroundColor(themeStore.accentColor())
                // The condition glyph changes as the forecast refreshes.
                .contentTransition(.symbolEffect(.replace))
            Text(weather?.temperature ?? Const.weatherPlaceholderValue)
                .font(themeStore.uiFont(size: Const.valueFontSize - 4, weight: .bold))
                .foregroundColor(themeStore.fontColor())
                .contentTransition(.numericText())
                .animation(Motion.Value.rollDigits, value: weather?.temperature)
            Text(caption)
                .font(themeStore.uiFont(size: Const.captionFontSize - 1, weight: .medium))
                .foregroundColor(themeStore.mutedTextColor())
                .lineLimit(1)
                .minimumScaleFactor(0.75)
            if let weather {
                details(weather)
            }
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(frostedTile(themeStore: themeStore))
        .overlay(tileBorder(isOn: false, themeStore: themeStore))
    }

    /// Today's high/low and, when reported, the chance of rain.
    @ViewBuilder
    private func details(_ weather: WeatherSnapshot) -> some View {
        VStack(spacing: 2) {
            Text("H \(weather.high)   L \(weather.low)")
                .foregroundColor(themeStore.secondaryTextColor())
            if let rainChance = weather.rainChance {
                HStack(spacing: 3) {
                    Image(systemName: "drop.fill")
                    Text(rainChance)
                }
                .foregroundColor(themeStore.mutedTextColor())
            }
        }
        .font(themeStore.uiFont(size: Const.captionFontSize - 1.5))
        .lineLimit(1)
        .minimumScaleFactor(0.7)
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
            .overlay(border)
        }
        .buttonStyle(PressableSurfaceStyle())
    }

    /// An armed confirm or a muted mic always draws its coloured state border, so
    /// it stays visible even under a borderless (0-thickness) theme. A resting
    /// tile defers to the theme border, matching the panel and search-result
    /// tiles, and disappears with them when the border is turned off.
    @ViewBuilder
    private var border: some View {
        let shape = RoundedRectangle(cornerRadius: Const.cornerRadius, style: .continuous)
        if confirming || micMuted {
            shape.strokeBorder(tint.opacity(launchpadAlertBorderOpacity), lineWidth: launchpadStateBorderWidth)
        } else if themeStore.borderLineWidth() > 0 {
            shape.strokeBorder(themeStore.borderColor(), lineWidth: themeStore.borderLineWidth())
        }
    }

    private var iconName: String {
        switch model.actionId {
        case LaunchpadActionID.mic: return micMuted ? "mic.slash.fill" : "mic.fill"
        case LaunchpadActionID.restart: return "arrow.clockwise"
        case LaunchpadActionID.shutdown: return "power"
        case LaunchpadActionID.screensaver: return "display"
        default: return "circle"
        }
    }
}

/// The Now Playing tile: track name plus a Pause/Play control (⌘P).
private struct LaunchpadMediaTile: View {
    let model: LaunchpadTileModel
    /// The system-wide now-playing track (any app), or nil when nothing plays.
    let snapshot: NowPlayingSnapshot?
    var themeStore: ThemeStore
    var onToggle: () -> Void
    var onPrevious: () -> Void
    var onNext: () -> Void

    private typealias Const = AppConstants.Launcher.Launchpad

    private var isPlaying: Bool { snapshot?.isPlaying ?? false }

    /// The current track, or an idle hint when nothing is playing.
    private var trackTitle: String {
        snapshot?.title ?? Const.nowPlayingIdleTitle
    }

    /// The secondary line: artist and/or owning app when known.
    private var subtitle: String? {
        [snapshot?.artist, snapshot?.app].compactMap { $0 }.filter { !$0.isEmpty }.first
    }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "music.note")
                .font(.system(size: 18, weight: .medium))
                .foregroundColor(themeStore.accentColor())
            VStack(alignment: .leading, spacing: 1) {
                Text(trackTitle)
                    .font(themeStore.uiFont(size: Const.titleFontSize, weight: .bold))
                    .foregroundColor(snapshot?.hasTrack == true ? themeStore.fontColor() : themeStore.mutedTextColor())
                    .lineLimit(1)
                if let subtitle {
                    Text(subtitle)
                        .font(themeStore.uiFont(size: Const.captionFontSize - 1))
                        .foregroundColor(themeStore.mutedTextColor())
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            transport
        }
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(frostedTile(themeStore: themeStore))
        .overlay(tileBorder(isOn: false, themeStore: themeStore))
    }

    private var transport: some View {
        HStack(spacing: 10) {
            transportButton("backward.fill", action: onPrevious)
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
            .buttonStyle(PressableSurfaceStyle())
            transportButton("forward.fill", action: onNext)
        }
    }

    private func transportButton(_ symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .semibold))
                .foregroundColor(themeStore.secondaryTextColor())
                .padding(6)
        }
        .buttonStyle(PressableSurfaceStyle())
    }
}

// MARK: - Shared tile chrome

private let launchpadOnTintOpacity = 0.22
/// Backs both the red confirm state and the amber muted-mic state, so the tile
/// takes its hue from `tint` rather than baking one in here.
private let launchpadAlertTintOpacity = 0.16

/// State-border chrome. A resting tile takes the user's theme border (see
/// `tileBorder` / `LaunchpadActionTile.border`); these values govern only the
/// coloured overlays a tile draws for a state the user just entered, an on-state
/// toggle or an armed confirm, which stay visible even under a borderless theme.
private let launchpadStateBorderWidth: CGFloat = 1
private let launchpadOnBorderOpacity = 0.35
private let launchpadAlertBorderOpacity = 0.3

/// The frosted surface every launchpad tile sits on: the same backdrop +
/// control-fill stack as `LauncherView.tileBackground(floats:)`. An optional tint
/// (accent for on-state toggles, caution for a confirming/muted action) layers on top.
func frostedTile(themeStore: ThemeStore, tint: Color? = nil, tintOpacity: Double = 0) -> some View {
    let radius = themeStore.surfaceCornerRadius(AppConstants.Launcher.Launchpad.cornerRadius)
    return ZStack {
        ThemedBackdrop(themeStore: themeStore, cornerRadius: radius)
        themeStore.controlFillColor()
        if let tint {
            tint.opacity(tintOpacity)
        }
    }
    .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
}

/// The border for a toggle or slot tile. An on-state toggle always draws its
/// accent outline so the active state stays legible under any theme; a resting
/// tile defers to the theme border (panel + search-result parity) and vanishes
/// with it when the border is turned off.
@ViewBuilder
private func tileBorder(isOn: Bool, themeStore: ThemeStore) -> some View {
    // Must track `frostedTile`'s radius exactly, or the outline and the fill
    // disagree at the corners under Liquid.
    let shape = RoundedRectangle(
        cornerRadius: themeStore.surfaceCornerRadius(AppConstants.Launcher.Launchpad.cornerRadius),
        style: .continuous
    )
    if isOn {
        shape.strokeBorder(themeStore.accentColor().opacity(launchpadOnBorderOpacity), lineWidth: launchpadStateBorderWidth)
    } else if themeStore.borderLineWidth() > 0 {
        shape.strokeBorder(themeStore.borderColor(), lineWidth: themeStore.borderLineWidth())
    }
}
