import AppKit
import Combine
import Observation

// Persistent NSStatusItem mini-timer.
//
// Visible only while a session is active. Click → opens the launcher
// to /pomo via a notification. PomoState is now @Observable (Combine
// publishers gone), so we get instant updates via `withObservationTracking`
// and ongoing once-per-second redraws via a Timer publisher.

@MainActor
final class PomoMenuBarItem {
    private var statusItem: NSStatusItem?
    private var tickCancellable: AnyCancellable?

    func install() {
        guard statusItem == nil else { return }
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.button?.image = NSImage(systemSymbolName: "timer", accessibilityDescription: "Pomodoro")
        item.button?.imagePosition = .imageLeft
        item.button?.title = ""
        item.button?.target = self
        item.button?.action = #selector(handleClick)
        statusItem = item

        // 1-Hz heartbeat keeps the visible remaining-time current.
        tickCancellable = Timer.publish(every: 1.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in self?.refresh() }

        refresh()
    }

    func uninstall() {
        if let statusItem {
            NSStatusBar.system.removeStatusItem(statusItem)
        }
        statusItem = nil
        tickCancellable?.cancel()
        tickCancellable = nil
    }

    @objc private func handleClick() {
        // Bring the launcher up and route to /pomo.
        NotificationCenter.default.post(name: .lookActivateLauncherRequested, object: nil)
        NotificationCenter.default.post(name: .lookOpenPomoRequested, object: nil)
    }

    private func refresh() {
        let state = PomoSharedState.shared
        guard let button = statusItem?.button else { return }

        // Wrap reads in withObservationTracking so the next change to any
        // of these properties re-runs refresh() immediately — even between
        // 1-Hz timer ticks. This keeps menu-bar updates feeling instant
        // when the user hits Start/Pause/Reset in the launcher.
        withObservationTracking {
            if let _ = state.activeIndex {
                button.title = " " + PomoCommand.formattedRemaining(state.secondsLeft)
                button.image = NSImage(systemSymbolName: "timer", accessibilityDescription: "Pomodoro")
                button.image?.isTemplate = true
            } else {
                button.title = ""
                button.image = nil
            }
        } onChange: { [weak self] in
            DispatchQueue.main.async { self?.refresh() }
        }
    }
}

extension Notification.Name {
    static let lookOpenPomoRequested = Notification.Name("look.openPomoRequested")
}
