import AppKit
import Combine
import Observation
import SwiftUI

// Persistent NSStatusItem mini-timer.
//
// Visible only while a session is active. Click → opens the launcher
// to /pomo via a notification. PomoState is now @Observable (Combine
// publishers gone), so we get instant updates via `withObservationTracking`
// and ongoing once-per-second redraws via a Timer publisher.
//
// Also surfaces in-app messages via a popover anchored to the status
// item button — used as a fallback for users who haven't granted macOS
// notification permission.

@MainActor
final class PomoMenuBarItem {
    private var statusItem: NSStatusItem?
    private var tickCancellable: AnyCancellable?
    private var messageObserver: NSObjectProtocol?
    private var popover: NSPopover?
    private var dismissWorkItem: DispatchWorkItem?

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

        // In-app message popover requests (phase transitions, ending-soon).
        messageObserver = NotificationCenter.default.addObserver(
            forName: .lookPomoStatusMessage,
            object: nil,
            queue: .main
        ) { [weak self] note in
            guard let self else { return }
            let title = note.userInfo?["title"] as? String ?? "Pomodoro"
            let subtitle = note.userInfo?["subtitle"] as? String
            // queue: .main delivers on the main thread, but the closure
            // type is @Sendable so we still need an explicit hop to
            // satisfy Swift 6's actor-isolation check.
            Task { @MainActor in
                self.showPopoverMessage(title: title, subtitle: subtitle)
            }
        }

        refresh()
    }

    func uninstall() {
        if let statusItem {
            NSStatusBar.system.removeStatusItem(statusItem)
        }
        statusItem = nil
        tickCancellable?.cancel()
        tickCancellable = nil
        if let messageObserver {
            NotificationCenter.default.removeObserver(messageObserver)
        }
        messageObserver = nil
        dismissWorkItem?.cancel()
        dismissWorkItem = nil
        popover?.performClose(nil)
        popover = nil
    }

    private func showPopoverMessage(title: String, subtitle: String?) {
        guard let button = statusItem?.button else { return }
        let popover = ensurePopover()
        let host = NSHostingController(rootView: PomoMenuBarMessageView(title: title, subtitle: subtitle))
        host.view.frame = NSRect(x: 0, y: 0, width: 260, height: subtitle == nil ? 50 : 70)
        popover.contentViewController = host
        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)

        // Auto-dismiss after a few seconds. Cancel any pending dismiss
        // first so a fresh message resets the clock instead of being
        // closed early by the previous one.
        dismissWorkItem?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.popover?.performClose(nil) }
        dismissWorkItem = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 4.0, execute: work)
    }

    private func ensurePopover() -> NSPopover {
        if let popover { return popover }
        let p = NSPopover()
        p.behavior = .transient
        popover = p
        return p
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
    static let lookPomoStatusMessage = Notification.Name("look.pomo.statusMessage")
}

private struct PomoMenuBarMessageView: View {
    let title: String
    let subtitle: String?

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            Image(systemName: "timer")
                .font(.title2)
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                if let subtitle {
                    Text(subtitle)
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity)
    }
}
