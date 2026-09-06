import AppKit
import OSLog
import SwiftUI

private let hotkeyLog = Logger(subsystem: "noah-code.Look", category: "hotkey")

extension LauncherView {
    func focusActiveInput(
        recoveryDelays: [Double] = [0.0, 0.04, 0.10],
        activateApp: Bool = true
    ) {
        if appUIState.showsThemeSettings {
            NotificationCenter.default.post(name: .lookFocusSettingsInputRequested, object: nil)
            return
        }

        focusRequestToken &+= 1
        let token = focusRequestToken

        if activateApp {
            NSApplication.shared.activate(ignoringOtherApps: true)
        }
        scheduleFocusRecovery(delays: recoveryDelays, token: token)
    }

    func activateLauncherModeAndFocus() {
        // Preserve the Settings screen across hide/recall. Leaving Settings is an
        // explicit action (Escape / close button -> closeSettingsPanel); recalling
        // Look should not silently drop the user back to home. Just restore focus
        // to the settings input (focusActiveInput routes there when in Settings).
        if appUIState.showsThemeSettings {
            focusActiveInput()
            return
        }

        if isCommandMode {
            pendingKillCandidate = nil
            if activeCommandAcceptsInput {
                focusActiveInput(recoveryDelays: [0.0, 0.04], activateApp: false)
            } else {
                isQueryFocused = false
            }
            return
        }

        focusActiveInput()
    }

    func scheduleFocusRecovery(delays: [Double], token: UInt64) {
        for delay in delays {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                guard token == focusRequestToken else { return }
                guard !appUIState.showsThemeSettings else { return }
                guard let window = launcherWindow() else { return }

                if !window.isVisible {
                    window.makeKeyAndOrderFront(nil)
                } else {
                    window.makeKey()
                    window.orderFront(nil)
                }

                if let responder = findEditableTextField(in: window.contentView) {
                    window.makeFirstResponder(responder)
                }

                isQueryFocused = true
            }
        }
    }

    func launcherWindow() -> NSWindow? {
        // The app has multiple NSWindows now (the launcher itself, the
        // menu-bar status item button window, the pomo popover anchor).
        // The status item / popover windows are tiny (≈16x24); the
        // launcher's minimum frame is 620x600 (set on ContentView). Use
        // a size threshold to filter them out.
        let isLauncherSized: (NSWindow) -> Bool = { w in
            w.frame.width >= 400 && w.frame.height >= 400
        }

        if let key = NSApplication.shared.keyWindow, isLauncherSized(key) {
            return key
        }

        let windows = NSApplication.shared.windows

        if let visibleLauncher = windows.first(where: { $0.isVisible && isLauncherSized($0) }) {
            return visibleLauncher
        }

        if let anyLauncher = windows.first(where: isLauncherSized) {
            return anyLauncher
        }

        // Fallbacks if for some reason no launcher-sized window exists yet.
        if let key = NSApplication.shared.keyWindow { return key }
        if let visible = windows.first(where: { $0.isVisible }) { return visible }
        return windows.first
    }

    func findEditableTextField(in view: NSView?) -> NSView? {
        guard let view else { return nil }

        if let textField = view as? NSTextField,
            textField.isEditable,
            !textField.isHidden,
            textField.alphaValue > 0.01
        {
            return textField
        }

        for subview in view.subviews {
            if let found = findEditableTextField(in: subview) {
                return found
            }
        }

        return nil
    }

    func toggleWindowVisibility() {
        let win = launcherWindow()
        let isActive = NSApplication.shared.isActive
        let visibleWindowCount = NSApplication.shared.windows.filter { $0.isVisible }.count
        hotkeyLog.notice(
            "toggle: isActive=\(isActive) windowCount=\(NSApplication.shared.windows.count) visibleCount=\(visibleWindowCount) keyWindow=\(NSApplication.shared.keyWindow != nil) winIsVisible=\(win?.isVisible ?? false) winIsHidden=\(NSApp.isHidden)"
        )

        if let window = win, window.isVisible && isActive {
            hotkeyLog.notice("toggle: -> HIDE branch")
            hideLauncherWindow()
            return
        }

        hotkeyLog.notice("toggle: -> SHOW branch")
        // Before the window is ordered front, so a dropped query is never painted.
        clearQueryIfRetentionExpired()
        // Re-arm the spawn cascade so the launchpad tiles and quick actions
        // settle in fresh on every open, not just the first per process.
        appearanceRevealToken &+= 1
        captureFrontmostAppForRestoreIfNeeded()
        _ = bridge.requestIndexRefresh()
        // Warm the on-device model the instant the launcher opens so the first
        // AI answer doesn't pay the cold-load cost while the user types.
        if themeStore.settings.aiEnabled {
            AIQueryRouter.shared.prewarm(themeStore.settings.aiProvider)
        }
        NSApplication.shared.unhide(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)

        if let window = launcherWindow() {
            positionOnActiveScreen(window)
            window.makeKeyAndOrderFront(nil)
            activateLauncherModeAndFocus()
            let frameStr = NSStringFromRect(window.frame)
            hotkeyLog.notice(
                "toggle: SHOW done - visible=\(window.isVisible) onActiveSpace=\(window.isOnActiveSpace) frame=\(frameStr, privacy: .public)"
            )
            return
        }

        openWindow(id: "main")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            NSApplication.shared.activate(ignoringOtherApps: true)
            if let window = launcherWindow() {
                positionOnActiveScreen(window)
                window.makeKeyAndOrderFront(nil)
            }
            activateLauncherModeAndFocus()
        }
    }

    /// The settings panel is a page of the AppKit-owned launcher window, so the
    /// menu command has to bring that window up itself. The SwiftUI Settings
    /// scene is only a command carrier and must not open a window of its own.
    /// Focus is left to the `showsThemeSettings` observer in LauncherView, which
    /// routes it to the settings input or back to the query field.
    func toggleThemeSettings() {
        appUIState.showsThemeSettings.toggle()
        revealLauncherWindowIfHidden()
    }

    private func revealLauncherWindowIfHidden() {
        guard let window = launcherWindow(), !window.isVisible else { return }

        // Only when nothing is stored yet: the launcher is frontmost whenever the
        // menu command fires, so recapturing here would drop the app the user
        // came from and leave hideLauncherWindow() with nothing to restore.
        if pidToRestoreOnHide == nil {
            captureFrontmostAppForRestoreIfNeeded()
        }
        clearQueryIfRetentionExpired()
        NSApplication.shared.unhide(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
        positionOnActiveScreen(window)
        window.makeKeyAndOrderFront(nil)
    }

    /// Places the launcher at its fixed position on whichever
    /// screen currently holds the mouse cursor. Called on every show so the
    /// launcher always appears on the display the user is working on (issue
    /// #260). The window is not draggable, so its position is owned entirely here.
    func positionOnActiveScreen(_ window: NSWindow) {
        let cursor = NSEvent.mouseLocation
        guard
            let screen = NSScreen.screens.first(where: { NSMouseInRect(cursor, $0.frame, false) })
                ?? NSScreen.main
        else { return }
        let frame = WindowAutoScale.spotlightFrame(on: screen)
        // Log the real screen + placement so the spotlight fraction can be
        // calibrated from actual displays rather than estimates.
        let topGap = screen.visibleFrame.maxY - frame.maxY
        hotkeyLog.debug(
            "position: visible=\(NSStringFromRect(screen.visibleFrame), privacy: .public) frame=\(NSStringFromRect(frame), privacy: .public) topGap=\(topGap, privacy: .public)"
        )
        window.setFrame(frame, display: true)
    }

    func hideLauncherWindow(restorePreviousApp: Bool = true) {
        guard let window = launcherWindow() else {
            hotkeyLog.notice("hide: no window")
            return
        }
        focusRequestToken &+= 1
        isQueryFocused = false
        // Don't leave a stale Empty Trash confirmation to reappear on next show.
        pendingEmptyTrashCount = nil
        pendingHideAppResult = nil
        // Levels do not survive a hide (§2.10). Their rows were produced live
        // from a row that may not even exist by the next open, and coming back
        // to a list with no visible way in would be worse than starting fresh.
        clearLevels()
        // The AI session intentionally survives hide/recall (Cmd+Space away and
        // back must not lose the conversation). Only Esc ends and archives it.
        let wasVisible = window.isVisible
        // Only a real visible-to-hidden transition winds the clock: a repeat
        // hide would otherwise restart it and defer the clear that was due.
        if wasVisible {
            lastHiddenAt = Date()
        }
        window.orderOut(nil)
        hotkeyLog.notice("hide: orderOut wasVisible=\(wasVisible) restore=\(restorePreviousApp)")

        // Preview bitmaps and highlighted text are the largest resident
        // buffers; drop them while hidden. They rebuild behind the preview
        // dwell, so reopening never shows the difference.
        HighlightedTextCache.purge()
        Task { await QuickLookPreviewService.shared.purge() }

        if restorePreviousApp {
            _ = reactivatePreviouslyFocusedAppIfNeeded()
        } else {
            pidToRestoreOnHide = nil
        }

        refreshClipboardMonitoringMode()
    }

    /// Called at launch and on config reload, so the show path only ever
    /// compares two dates.
    func reloadQueryRetentionPolicy() {
        let path = ConfigPathResolver.resolvedPath()
        guard let raw = try? String(contentsOfFile: path, encoding: .utf8) else {
            queryRetentionSeconds = AppConstants.Launcher.QueryRetention.defaultSeconds
            return
        }
        queryRetentionSeconds = QueryRetentionPolicy.resolveSeconds(
            from: ConfigFileLines.keyValues(raw))
    }

    /// The stamp is consumed either way, so an early show cannot leave a later
    /// one clearing on a stale hide.
    func clearQueryIfRetentionExpired() {
        let expired = QueryRetentionPolicy.shouldClear(
            hiddenAt: lastHiddenAt, seconds: queryRetentionSeconds)
        lastHiddenAt = nil
        guard expired else { return }
        // Levels are already gone. Command mode carries its own input, which
        // clearing `query` alone would strand.
        exitCommandMode()
        if isAIMode {
            // The conversation survives hide/recall by design; only the stale
            // draft goes. Silently, or the compose handler cancels the chat's work.
            clearQuerySilently()
        } else {
            query = ""
        }
    }

    func captureFrontmostAppForRestoreIfNeeded() {
        guard let frontmost = NSWorkspace.shared.frontmostApplication else {
            pidToRestoreOnHide = nil
            return
        }

        if frontmost.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            pidToRestoreOnHide = nil
            return
        }

        pidToRestoreOnHide = frontmost.processIdentifier
    }

    @discardableResult
    func reactivatePreviouslyFocusedAppIfNeeded() -> Bool {
        guard let pid = pidToRestoreOnHide else { return false }
        pidToRestoreOnHide = nil
        guard pid != ProcessInfo.processInfo.processIdentifier else { return false }
        guard let app = NSRunningApplication(processIdentifier: pid) else { return false }
        guard !app.isTerminated else { return false }

        DispatchQueue.main.asyncAfter(deadline: .now() + Self.postHideActivationDelay) {
            _ = app.activate()
        }
        return true
    }

    func refreshClipboardMonitoringMode() {
        let isVisible = launcherWindow()?.isVisible ?? false
        if NSApplication.shared.isActive && isVisible {
            clipboardStore.setMonitoringMode(.foreground)
        } else {
            clipboardStore.setMonitoringMode(.background)
        }
    }
}
