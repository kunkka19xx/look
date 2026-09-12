import SwiftUI

/// The empty-state launchpad: the bento of tiles shown below the search bar
/// when the query is empty (see "Empty State Spec"). Tile sizes, labels,
/// mnemonics and cells come from the shared `look_qactions` catalog as the
/// user's `~/.look/super-actions.toml` arranged them; this view places them
/// and renders live state, except the L slot, which reads the Todo / Pomo
/// stores.
///
/// The drawing can also be edited from here: hold a tile until it lifts, then
/// drag it onto another to trade places, or into a gap to move there. The
/// drop is reported out and written to the file, so a drag and a text editor
/// are two ways of making the same edit. A drop that would cover another tile
/// or leave the grid is refused, and the tile springs back.
struct EmptyStateLaunchpadView: View {
    let tiles: [LaunchpadTileModel]
    /// The shape the drawing declared, when the payload carries one.
    var shape: LaunchpadGrid.Shape?
    var controller: LaunchpadController
    var themeStore: ThemeStore
    /// Changes each time the launcher opens, replaying the spawn cascade.
    var revealToken: UInt64 = 0
    /// A drag settled on a new arrangement, which the owner writes to the
    /// drawing and reads back.
    var onArrange: ([LaunchpadTileModel]) -> Void = { _ in }

    private typealias Const = AppConstants.Launcher.Launchpad

    /// Rearranging springs tiles around, which is exactly what Reduce Motion
    /// asks apps not to do. `Motion.Press` and the reveal cascade already
    /// check it; this keeps the grid in step.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// Nil under Reduce Motion, so tiles change cells instantly.
    private var reflow: Animation? {
        reduceMotion ? nil : Motion.Reorder.animation
    }

    /// Drag coordinates are read in the grid's own space, so a tile's cell and
    /// the pointer are measured against the same origin.
    private static let gridSpace = "look.launchpad.grid"

    /// Where the held tile would land if dropped now, or nil when the pointer
    /// proposes nothing that fits - the grid then shows `tiles`, unchanged.
    @State private var preview: [LaunchpadTileModel]?
    @State private var drag: DragSession?
    /// When the last drag ended, so the mouse-up that ended it cannot also fire
    /// the tile's action.
    @State private var droppedAt = Date.distantPast

    /// A tile held and being dragged. Deliberately holds no live translation:
    /// that lives in the held tile's own `@GestureState` so a pointer move
    /// repaints one tile instead of re-running this view's whole body. All this
    /// carries is what the *rest* of the grid needs to know - which tile is up,
    /// and the corner it was lifted from, which the tile stays pinned to while
    /// the others move.
    private struct DragSession {
        let tileID: String
        let liftOrigin: CGPoint
    }

    /// The arrangement to render.
    private var current: [LaunchpadTileModel] { preview ?? tiles }

    private var showsNowPlaying: Bool { tiles.contains(role: .media) }

    /// Cells to points. Built from `tiles`, not the preview: a trade never
    /// changes the shape, and the grid must not resize under a drag.
    private var grid: LaunchpadGrid {
        LaunchpadGrid(tiles: tiles, declared: shape, rowHeight: Const.rowHeight, gap: Const.gap)
    }

    private var gridShape: LaunchpadGrid.Shape {
        LaunchpadGrid.Shape(columns: grid.columns, rows: grid.rows)
    }

    var body: some View {
        GeometryReader { geo in
            layout(width: geo.size.width)
        }
        .frame(height: totalHeight)
        .padding(.top, Const.outerTopPadding)
        // The launcher window is only ordered out, never torn down, so
        // `onAppear` fires once per process. A drag interrupted by the launcher
        // closing under it - the hotkey or Esc while the mouse is still down -
        // would otherwise strand a tile lifted, the rest dimmed, and every click
        // swallowed for the life of the process. `revealToken` changes on every
        // open, which makes it the one signal that can undo that. The in-flight
        // preview is dropped rather than kept: it was never saved, and reopening
        // onto a half-moved grid is worse than reopening onto the one that was
        // last written.
        .onChange(of: revealToken) { _, _ in
            drag = nil
            preview = nil
            droppedAt = .distantPast
        }
        // A reload landed: the file was edited, or the owner just wrote a drop
        // back and read it again. Adopted unless a drag is driving the preview,
        // which would fight the pointer.
        .onChange(of: tiles) { _, _ in
            if drag == nil { preview = nil }
        }
        // Poll system now-playing while the launchpad is on screen, so external
        // changes (pausing in a browser) are reflected. Cancelled on disappear.
        // Keyed on the tile so an edit adding or removing it starts or stops
        // the poll, rather than waking every few seconds to feed nothing.
        //
        // Skipped while a tile is up: the read shells out to `osascript` (see
        // `SystemNowPlaying`), and a process spawn plus the repaint its result
        // triggers are both felt as a hitch mid-drag. Nothing is lost - the
        // next tick picks the track up a moment later.
        .task(id: showsNowPlaying) {
            guard showsNowPlaying else { return }
            while !Task.isCancelled {
                if drag == nil {
                    await controller.refreshNowPlaying()
                }
                try? await Task.sleep(for: .seconds(Const.nowPlayingPollSeconds))
            }
        }
    }

    // MARK: Geometry

    private var totalHeight: CGFloat { grid.height }

    /// The grid cell under `point`, or nil outside the grid. The gap after a
    /// cell counts as that cell.
    private func cell(at point: CGPoint, width: CGFloat) -> LaunchpadArrangement.Cell? {
        let grid = self.grid
        let col = Int(floor(point.x / (grid.cellWidth(total: width) + Const.gap)))
        let row = Int(floor(point.y / (Const.rowHeight + Const.gap)))
        guard (0..<grid.columns).contains(col), (0..<grid.rows).contains(row) else { return nil }
        return LaunchpadArrangement.Cell(col: col, row: row)
    }

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
        // Computed once per pass rather than per tile: each answer is a trial
        // trade, and twelve of those on every repaint adds up.
        let blocked = blockedTileIDs()
        return ZStack(alignment: .topLeading) {
            // Reading order, because that is the order the core sends and the
            // arrangement keeps. It is also the order the entrance cascade
            // wants, so the index is the cascade position - no second table of
            // who animates when.
            ForEach(Array(current.enumerated()), id: \.element.actionId) { index, model in
                let box = grid.frame(for: model, totalWidth: width)
                LaunchpadTileSlot(
                    frame: box.size,
                    anchor: anchor(for: model, box: box),
                    isLifted: drag?.tileID == model.actionId,
                    isBlocked: blocked.contains(model.actionId),
                    revealIndex: index,
                    revealToken: revealToken,
                    gridSpace: Self.gridSpace,
                    reduceMotion: reduceMotion,
                    onLift: { lift(model, box: box) },
                    onMove: { start, location in moveDrag(from: start, to: location, width: width) },
                    onDrop: drop
                ) {
                    tileContent(model)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .coordinateSpace(name: Self.gridSpace)
    }

    /// Where a tile's cell sits, in grid space. A held tile keeps the corner it
    /// was lifted from: the preview moves it to its new cell the moment a trade
    /// is proposed, and following that would tear it out from under the
    /// pointer. Its live translation is added by the slot itself.
    private func anchor(for model: LaunchpadTileModel, box: CGRect) -> CGPoint {
        if let drag, drag.tileID == model.actionId {
            return drag.liftOrigin
        }
        return box.origin
    }

    /// Tiles the held one cannot trade places with, dimmed so a refusal reads
    /// before the drop rather than after. Empty when nothing is held.
    private func blockedTileIDs() -> Set<String> {
        guard let drag else { return [] }
        let shape = gridShape
        return Set(
            tiles
                .filter { $0.actionId != drag.tileID }
                .filter { LaunchpadArrangement.swapping(drag.tileID, with: $0.actionId, in: tiles, shape: shape) == nil }
                .map(\.actionId)
        )
    }

    @ViewBuilder
    private func tileContent(_ model: LaunchpadTileModel) -> some View {
        switch model.role {
        case .toggle:
            LaunchpadToggleTile(
                model: model,
                isOn: controller.isOn(model.actionId),
                themeStore: themeStore
            ) { activate(model) }
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
            ) { activate(model) }
        case .media:
            LaunchpadMediaTile(
                model: model,
                snapshot: controller.nowPlaying,
                themeStore: themeStore,
                onToggle: { activate(model) },
                onPrevious: { if isActivatable { controller.nowPlayingPrevious() } },
                onNext: { if isActivatable { controller.nowPlayingNext() } }
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
            ) { activate(model) }
        }
    }

    // MARK: Rearranging

    private func lift(_ model: LaunchpadTileModel, box: CGRect) {
        guard drag == nil else { return }
        withAnimation(reflow) {
            drag = DragSession(tileID: model.actionId, liftOrigin: box.origin)
        }
    }

    /// Re-draws the grid when the pointer proposes a different landing. Called
    /// on every pointer move but writes nothing on most of them: the held
    /// tile's own travel is drawn by its slot, so this view repaints only when
    /// the proposal changes.
    private func moveDrag(from start: CGPoint, to location: CGPoint, width: CGFloat) {
        guard let session = drag,
              let held = tiles.first(where: { $0.actionId == session.tileID })
        else { return }
        let proposal = landing(for: held, grabbedAt: start, pointer: location, width: width)
        // A proposal that is the committed arrangement is no preview at all.
        let next = proposal == tiles ? nil : proposal
        guard next != preview else { return }
        withAnimation(reflow) { preview = next }
    }

    /// Where the held tile would go: trading places with the tile under the
    /// pointer, else into the gap there. Judged against `tiles`, the committed
    /// drawing, never the preview - so dragging a tile around does not leave a
    /// trail of trades behind it. Nil when neither fits.
    private func landing(
        for held: LaunchpadTileModel,
        grabbedAt start: CGPoint,
        pointer: CGPoint,
        width: CGFloat
    ) -> [LaunchpadTileModel]? {
        guard let under = cell(at: pointer, width: width) else { return nil }
        let shape = gridShape
        if let target = LaunchpadArrangement.tile(at: under, in: tiles), target.actionId != held.actionId {
            return LaunchpadArrangement.swapping(held.actionId, with: target.actionId, in: tiles, shape: shape)
        }
        // A gap. Which of its own cells the tile was picked up by decides where
        // its corner lands, so a wide tile drops where it looks like it will
        // rather than where its corner is.
        let grabbed = cell(at: start, width: width) ?? LaunchpadArrangement.Cell(col: held.col, row: held.row)
        let byCol = min(max(grabbed.col - held.col, 0), held.columnSpan - 1)
        let byRow = min(max(grabbed.row - held.row, 0), held.rowSpanCount - 1)
        let origin = LaunchpadArrangement.Cell(col: under.col - byCol, row: under.row - byRow)
        return LaunchpadArrangement.moving(held.actionId, to: origin, in: tiles, shape: shape)
    }

    /// Puts the held tile down. Idempotent: both end-of-gesture paths call it,
    /// and whichever arrives first does the work.
    ///
    /// Releasing without having moved is a no-op, not a click: once the hold has
    /// lifted a tile the press belongs to the rearrange, so `droppedAt` swallows
    /// the mouse-up either way rather than letting a slow press on Wi-Fi both
    /// lift it and toggle it.
    private func drop() {
        guard drag != nil else { return }
        withAnimation(reflow) { drag = nil }
        droppedAt = Date()
        // A hold that lifted a tile and put it back is not an edit, and should
        // not cost a file write.
        guard let settled = preview else { return }
        // Kept on screen as the preview until the owner echoes the written
        // drawing back through `tiles`, so the tile does not flash home and
        // back while the file is written.
        onArrange(settled)
    }

    // MARK: Activation

    /// False while a tile is held, and for a beat after one is dropped: the
    /// mouse-up that ends a drag reaches the tile's button as an ordinary click,
    /// and rearranging Wi-Fi should not also turn it off.
    private var isActivatable: Bool {
        drag == nil && Date().timeIntervalSince(droppedAt) > Const.reorderClickSuppressSeconds
    }

    private func activate(_ model: LaunchpadTileModel) {
        guard isActivatable else { return }
        controller.activate(model)
    }
}

// MARK: - Tile slot

/// One tile in its cell, and the gesture that rearranges it.
///
/// This exists to keep a drag off the grid's critical path. The launchpad's
/// tiles are each backed by a real AppKit blur / glass view, so repainting all
/// of them is expensive - and holding the drag translation in the *parent*
/// meant exactly that on every pointer move. Owning the translation here
/// confines a move to the one tile that is actually travelling; the grid above
/// only repaints when the proposal changes, a handful of times per drag.
private struct LaunchpadTileSlot<Content: View>: View {
    let frame: CGSize
    /// The cell's corner in grid space, before this tile's own travel.
    let anchor: CGPoint
    let isLifted: Bool
    let isBlocked: Bool
    let revealIndex: Int
    let revealToken: UInt64
    let gridSpace: String
    let onLift: () -> Void
    /// Where the drag began and where the pointer is now, both in grid space.
    let onMove: (CGPoint, CGPoint) -> Void
    let onDrop: () -> Void
    let content: Content

    private typealias Const = AppConstants.Launcher.Launchpad

    /// How far the pointer has carried this tile. Reset by SwiftUI when the
    /// gesture ends - animated, so the tile springs into its cell instead of
    /// snapping back the instant the mouse comes up. It shares `Motion.Reorder`
    /// with the anchor's own animation in `drop()`, so the two compose into one
    /// settle rather than reading as two.
    ///
    /// The reset transaction is fixed when the property wrapper is built, which
    /// is why Reduce Motion is passed in rather than read from the environment:
    /// there is no `@Environment` yet at that point.
    @GestureState private var travel: CGSize

    /// True only while this tile's rearrange gesture is live.
    @GestureState private var isRearranging = false

    init(
        frame: CGSize,
        anchor: CGPoint,
        isLifted: Bool,
        isBlocked: Bool,
        revealIndex: Int,
        revealToken: UInt64,
        gridSpace: String,
        reduceMotion: Bool,
        onLift: @escaping () -> Void,
        onMove: @escaping (CGPoint, CGPoint) -> Void,
        onDrop: @escaping () -> Void,
        @ViewBuilder content: () -> Content
    ) {
        self.frame = frame
        self.anchor = anchor
        self.isLifted = isLifted
        self.isBlocked = isBlocked
        self.revealIndex = revealIndex
        self.revealToken = revealToken
        self.gridSpace = gridSpace
        self.onLift = onLift
        self.onMove = onMove
        self.onDrop = onDrop
        self.content = content()
        _travel = GestureState(
            wrappedValue: .zero,
            resetTransaction: reduceMotion
                ? Transaction()
                : Transaction(animation: Motion.Reorder.animation)
        )
    }

    var body: some View {
        content
            .frame(width: frame.width, height: frame.height)
            .scaleEffect(isLifted ? Const.reorderLiftScale : 1)
            .shadow(
                color: .black.opacity(isLifted ? Const.reorderLiftShadowOpacity : 0),
                radius: isLifted ? Const.reorderLiftShadowRadius : 0
            )
            .opacity(isBlocked ? Const.reorderBlockedOpacity : 1)
            // On the container: symbol effects reach the images inside.
            .symbolEffect(.bounce, value: revealToken)
            .spawnReveal(index: revealIndex, token: revealToken)
            // Placement goes on last, so the lift and the spawn cascade both
            // scale a tile about its own centre. Offsetting first would leave
            // them scaling about the grid's top-left corner instead.
            .offset(x: anchor.x + travel.width, y: anchor.y + travel.height)
            // The held tile rides above the ones it is dragged over.
            .zIndex(isLifted ? 1 : 0)
            // Simultaneous, not exclusive: the tile's own button keeps handling
            // plain clicks, and the grid drops the one that ends a drag.
            .simultaneousGesture(reorderGesture)
            // The gesture going away is the one end-of-drag signal that always
            // arrives, so the drop is driven from here rather than `onEnded`.
            .onChange(of: isRearranging) { _, live in
                if !live { onDrop() }
            }
    }

    /// Hold to pick the tile up, then drag it onto another to trade places or
    /// into a gap to move there. The long press is what keeps a click a click:
    /// released early the sequence never completes, so nothing was ever lifted.
    ///
    /// `updating` is what puts the tile back down, not `onEnded`. A sequenced
    /// drag that never moves - a press held just past the hold threshold and
    /// released on the spot - ends without ever calling `onEnded`, which would
    /// strand the grid mid-drag: the tile stays lifted, every other tile stays
    /// dimmed, and the whole strip goes inert. `@GestureState` is reset by
    /// SwiftUI itself when a gesture ends *or* is cancelled, so it cannot be
    /// stranded. `onEnded` is kept as the prompt path for a drag that did move.
    private var reorderGesture: some Gesture {
        LongPressGesture(
            minimumDuration: Const.reorderHoldSeconds,
            maximumDistance: Const.reorderHoldSlop
        )
        .sequenced(before: DragGesture(minimumDistance: 0, coordinateSpace: .named(gridSpace)))
        .updating($isRearranging) { _, live, _ in live = true }
        // The tile follows the drag's own translation rather than the pointer's
        // absolute position, so it moves exactly as far as the pointer does
        // instead of snapping its corner under the cursor at the first pixel.
        .updating($travel) { value, moved, _ in
            if case .second(true, let drag?) = value {
                moved = drag.translation
            }
        }
        .onChanged { value in
            switch value {
            case .first(true):
                onLift()
            case .second(true, let drag):
                // The hold can complete straight into the drag phase without a
                // `.first` update of its own, so lift here too; the grid makes
                // it a no-op once the tile is already held.
                onLift()
                if let drag { onMove(drag.startLocation, drag.location) }
            default:
                break
            }
        }
        .onEnded { _ in onDrop() }
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
