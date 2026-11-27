import SwiftUI

extension LauncherView {
    static let pinBannerDuration = 2.0

    func refreshBarSlot() {
        barSlotService.refresh(slot: barSlot, pinnedIDs: themeStore.pinnedAppIDs, script: themeStore.barScript)
    }

    @ViewBuilder
    var barSlotView: some View {
        switch barSlot {
        case .running:
            if !runningAppsService.items.isEmpty {
                appStrip(runningAppTiles)
            }
        case .pinned:
            if !barSlotService.shownPins.isEmpty {
                appStrip(pinnedAppTiles)
            }
        case .todo:
            let line = todayLine
            BarTextSlotView(
                lead: line.task, note: line.note, themeStore: themeStore,
                onTap: openTodo)
        case .script:
            if let line = barSlotService.scriptLine(for: themeStore.barScript) {
                BarTextSlotView(lead: line, note: nil, themeStore: themeStore)
            }
        case .none:
            EmptyView()
        }
    }

    private func appStrip(_ tiles: [AppStripTile]) -> some View {
        AppStripView(
            tiles: tiles,
            themeStore: themeStore,
            onActivate: { key in _ = activateBarSlot(forKey: key) })
    }

    private var runningAppTiles: [AppStripTile] {
        runningAppsService.items.map { app in
            AppStripTile(
                id: String(app.id), name: app.name, icon: app.icon,
                isActive: runningAppsService.activePID == app.id)
        }
    }

    private var pinnedAppTiles: [AppStripTile] {
        barSlotService.shownPins.map { pin in
            AppStripTile(id: pin.id, name: pin.title ?? pin.id, icon: pin.path.map(RowIconCache.icon(forFile:)))
        }
    }

    private var todayLine: BarTodoLine {
        let state = TodoSharedState.shared
        let stat = state.todayStat
        return BarTodoLine(
            done: stat.done, total: stat.total, open: state.todayOpenTasks, pick: barSlotService.todoPick)
    }

    @discardableResult
    func openPinnedApp(at position: Int) -> Bool {
        let pins = barSlotService.shownPins
        guard pins.indices.contains(position), let path = pins[position].path else { return false }
        openApp(path: path, candidateID: pins[position].id)
        return true
    }

    func isPinnable(_ result: LauncherResult) -> Bool {
        result.kind == .app && result.id.hasPrefix(BarSlot.pinnableIDPrefix)
    }

    func toggleSelectedPin() {
        guard let row = actionableSelectedResult(), isPinnable(row) else { return }
        let message: String
        let style: BannerStyle
        switch themeStore.togglePinnedApp(row.id) {
        case .pinned:
            (message, style) = ("\(row.title) pinned to the bar", .success)
        case .unpinned:
            (message, style) = ("\(row.title) unpinned", .info)
        case .full:
            (message, style) = ("The bar holds \(EngineBridge.maxPinnedApps) apps; unpin one first", .info)
        case nil:
            (message, style) = ("Pin failed: the config could not be written", .error)
        }
        showBanner(message, style: style, duration: Self.pinBannerDuration)
        refreshQuickActions()
    }
}
