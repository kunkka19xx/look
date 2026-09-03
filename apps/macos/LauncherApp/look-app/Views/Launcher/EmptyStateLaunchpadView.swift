import SwiftUI

/// The empty-state launchpad: a 6-column bento of L/M/S tiles shown below the
/// search bar when the query is empty (see "Empty State Spec"). Tile order,
/// sizes, labels, and mnemonics come from the shared `look_qactions` catalog;
/// this view lays them out and (in this pass) renders mock state, except the L
/// slot, which reads the live Todo / Pomo stores.
struct EmptyStateLaunchpadView: View {
    let tiles: [LaunchpadTileModel]
    /// The shape the drawing declared, when the payload carries one.
    var shape: LaunchpadGrid.Shape?
    var controller: LaunchpadController
    var themeStore: ThemeStore
    /// Changes each time the launcher opens, replaying the spawn cascade.
    var revealToken: UInt64 = 0

    private typealias Const = AppConstants.Launcher.Launchpad

    private var showsNowPlaying: Bool { tiles.contains(role: .media) }

    /// Cells to points. Cheap enough to rebuild; it is four numbers.
    private var grid: LaunchpadGrid {
        LaunchpadGrid(tiles: tiles, declared: shape, rowHeight: Const.rowHeight, gap: Const.gap)
    }

    var body: some View {
        GeometryReader { geo in
            layout(width: geo.size.width)
        }
        .frame(height: totalHeight)
        .padding(.top, Const.outerTopPadding)
        // Poll system now-playing while the launchpad is on screen, so external
        // changes (pausing in a browser) are reflected. Cancelled on disappear.
        // Keyed on the tile so an edit adding or removing it starts or stops
        // the poll, rather than waking every few seconds to feed nothing.
        .task(id: showsNowPlaying) {
            guard showsNowPlaying else { return }
            while !Task.isCancelled {
                await controller.refreshNowPlaying()
                try? await Task.sleep(for: .seconds(Const.nowPlayingPollSeconds))
            }
        }
    }

    // MARK: Geometry

    private var totalHeight: CGFloat { grid.height }

    // MARK: Layout
    //
    // Every tile is placed at the cell the core resolved for it. This used to be
    // composed by hand - nested stacks naming each tile in the order the catalog
    // happened to return them - because SwiftUI has no cell spanning and the
    // arrangement was known in advance. It is not known in advance any more:
    // ~/.look/super-actions.toml decides it, so the view offsets and sizes each tile
    // from its own coordinates and never reconstructs an arrangement.

    private func layout(width: CGFloat) -> some View {
        let grid = self.grid
        return ZStack(alignment: .topLeading) {
            // Reading order, because that is the order the core sends. It is
            // also the order the entrance cascade wants, so the index is the
            // cascade position - no second table of who animates when.
            ForEach(Array(tiles.enumerated()), id: \.element.actionId) { index, model in
                let box = grid.frame(for: model, totalWidth: width)
                tileContent(model)
                    .frame(width: box.width, height: box.height)
                    // On the container: symbol effects reach the images inside.
                    .symbolEffect(.bounce, value: revealToken)
                    .spawnReveal(index: index, token: revealToken)
                    .offset(x: box.minX, y: box.minY)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
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
            // Was placed by its own branch of the old hand-composed layout, and
            // reached this switch only to render nothing. It is a tile like the
            // rest now, so it can be moved, resized or left out of the drawing.
            LaunchpadLSlotView(themeStore: themeStore)
        case .custom:
            LaunchpadCustomTile(
                model: model,
                value: controller.customValue(for: model.actionId),
                isPressable: model.pressable,
                confirming: controller.pendingConfirmActionID == model.actionId,
                themeStore: themeStore
            ) { controller.activate(model) }
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
                    // Theme and Keep Awake swap glyph on flip.
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
                // The glyph gains and loses its bolt as charging changes.
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

/// A tile the user declared in `~/.look/super-actions.toml`. Same anatomy as the
/// tiles beside it; how much shows is how big the user drew it. Placeholder
/// until the first run resolves, like Battery and Weather.
private struct LaunchpadCustomTile: View {
    let model: LaunchpadTileModel
    /// Nil until the tile's command has produced something.
    let value: LaunchpadTileValue?
    /// Whether pressing it does anything. A tile with no `press` is a readout.
    let isPressable: Bool
    /// Armed by a first press. There is no dialog: if the tile does not show
    /// it, nothing does.
    let confirming: Bool
    var themeStore: ThemeStore
    let onPress: () -> Void

    private typealias Const = AppConstants.Launcher.Launchpad

    /// One cell fits the headline alone.
    private var showsDetail: Bool { model.rowSpanCount > 1 || model.columnSpan > 1 }

    /// The command's caption wins over the tile's name - the rule Weather uses,
    /// showing the condition rather than the word "Weather". Unless the tile
    /// has a key: that letter lives in the name.
    private var label: String {
        let text = model.mnemonic == nil ? (value?.caption ?? model.title) : model.title
        // Uppercased to sit level with BATTERY and CLEAR beside it.
        return text.uppercased()
    }

    /// The caption line, only when it is not already doing duty as the label.
    private var detailCaption: String? {
        guard showsDetail, let caption = value?.caption, model.mnemonic != nil else { return nil }
        return caption.uppercased()
    }

    /// A tile that only acts, drawn like Mic and Screensaver. A placeholder
    /// would be a permanent "--" for something never going to fill in.
    private var button: some View {
        VStack(spacing: 8) {
            Image(systemName: value?.icon ?? model.icon ?? Const.customTileFallbackIcon)
                .font(.system(size: 24, weight: .medium))
                .foregroundColor(confirming ? themeStore.dangerColor() : themeStore.accentColor())
            mnemonicText(
                confirming ? (model.confirm ?? "Confirm?") : model.title,
                mnemonic: confirming ? nil : model.mnemonic,
                font: themeStore.uiFont(size: Const.titleFontSize, weight: .semibold),
                base: confirming ? themeStore.dangerColor() : themeStore.fontColor(),
                highlight: themeStore.warningColor()
            )
            .lineLimit(1)
        }
        .padding(.horizontal, 10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(
            frostedTile(
                themeStore: themeStore,
                tint: confirming ? themeStore.dangerColor() : nil,
                tintOpacity: confirming ? launchpadAlertTintOpacity : 0
            )
        )
        .overlay(tileBorder(isOn: false, themeStore: themeStore))
    }

    private var body_: some View {
        // Battery's anatomy, and the Linux/Windows strip's: icon in a column of
        // its own, caption and value stacked beside it.
        HStack(spacing: 10) {
            if let icon = (value?.icon ?? model.icon), !icon.isEmpty {
                Image(systemName: icon)
                    .font(.system(size: 18, weight: .medium))
                    .foregroundColor(themeStore.accentColor())
                    .contentTransition(.symbolEffect(.replace))
            }

            VStack(alignment: .leading, spacing: 2) {
                mnemonicText(
                    confirming ? (model.confirm?.uppercased() ?? "CONFIRM?") : label,
                    mnemonic: confirming ? nil : model.mnemonic,
                    font: themeStore.uiFont(size: Const.captionFontSize - 1, weight: .medium),
                    base: themeStore.mutedTextColor(),
                    highlight: themeStore.warningColor()
                )
                .lineLimit(1)

                Text(value?.value ?? Const.infoPlaceholderValue)
                    .font(themeStore.uiFont(size: Const.valueFontSize - 6, weight: .bold))
                    .foregroundColor(themeStore.fontColor())
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                    .contentTransition(.numericText())
                    .animation(Motion.Value.rollDigits, value: value?.value ?? "")

                if showsDetail {
                    if let detailCaption {
                        Text(detailCaption)
                            .font(themeStore.uiFont(size: Const.captionFontSize - 1, weight: .medium))
                            .foregroundColor(themeStore.mutedTextColor())
                            .lineLimit(1)
                    }
                    // Clipped, not scrolled: a tile is a glance, not a pane.
                    let extra = Array((value?.lines ?? []).prefix(3))
                    ForEach(Array(extra.enumerated()), id: \.offset) { _, line in
                        Text(line)
                            .font(themeStore.uiFont(size: Const.smallLabelFontSize - 0.5, weight: .regular))
                            .foregroundColor(themeStore.secondaryTextColor())
                            .lineLimit(1)
                    }
                }
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(frostedTile(themeStore: themeStore))
        .overlay(tileBorder(isOn: value?.isOn ?? false, themeStore: themeStore))
    }

    @ViewBuilder
    private var face: some View {
        if model.hasValue { body_ } else { button }
    }

    var body: some View {
        // Decided by the tile, not by whether a value has arrived yet.
        if isPressable {
            Button(action: onPress) { face }
                .buttonStyle(PressableSurfaceStyle())
        } else {
            face
        }
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
                    confirming ? (model.confirm ?? "Confirm?") : model.title,
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
        // Same scaled radius as `frostedTile`, or the outline cuts the corners.
        let shape = RoundedRectangle(
            cornerRadius: themeStore.tileRadius,
            style: .continuous
        )
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
                .clipShape(RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous))
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
func frostedTile(
    themeStore: ThemeStore,
    cornerRadius: CGFloat? = nil,
    /// Floating surfaces sit straight on the desktop with no window backdrop
    /// behind them, so they have to sample it directly. `.withinWindow` would
    /// find nothing there and render a flat wash instead of a blur.
    blendingMode: NSVisualEffectView.BlendingMode = .behindWindow,
    tint: Color? = nil,
    tintOpacity: Double = 0
) -> some View {
    let radius = cornerRadius ?? themeStore.tileRadius
    return ZStack {
        ThemedBackdrop(themeStore: themeStore, blendingMode: blendingMode, cornerRadius: radius)
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
    // Must track `frostedTile`'s radius, or the two disagree at the corners.
    let shape = RoundedRectangle(
        cornerRadius: themeStore.tileRadius,
        style: .continuous
    )
    if isOn {
        shape.strokeBorder(themeStore.accentColor().opacity(launchpadOnBorderOpacity), lineWidth: launchpadStateBorderWidth)
    } else if themeStore.borderLineWidth() > 0 {
        shape.strokeBorder(themeStore.borderColor(), lineWidth: themeStore.borderLineWidth())
    }
}
