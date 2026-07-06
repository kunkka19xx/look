import AppKit
import Observation
import SwiftUI

// Persistent NSStatusItem mini-timer.
//
// Visible only while a session is active. Click → opens the launcher
// to /pomo via a notification. PomoState is @Observable, so refresh()
// re-runs whenever a tracked property mutates - no separate heartbeat
// timer is needed.
//
// Also surfaces in-app messages via a popover anchored to the status
// item button - used as a fallback for users who haven't granted macOS
// notification permission.

@MainActor
final class PomoMenuBarItem {
    private var statusItem: NSStatusItem?
    private var messageObserver: NSObjectProtocol?
    private var popover: NSPopover?
    private var dismissWorkItem: DispatchWorkItem?
    // Coalesces overlapping refresh() calls. Without this, each fired
    // observer schedules its own async refresh, which installs another
    // observer - repeat ticks compound into N refreshes per second and
    // the SF Symbol re-rasterization saturates the main thread.
    private var refreshScheduled = false
    private var timerImage: NSImage?

    func install() {
        guard statusItem == nil else { return }
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let image = NSImage(systemSymbolName: "timer", accessibilityDescription: "Pomodoro")
        image?.isTemplate = true
        timerImage = image
        item.button?.image = nil
        item.button?.imagePosition = .imageLeft
        item.button?.title = ""
        item.button?.target = self
        item.button?.action = #selector(handleClick)
        statusItem = item

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
        refreshScheduled = false
        let state = PomoSharedState.shared
        guard let button = statusItem?.button else { return }

        // Reading inside withObservationTracking re-fires onChange when
        // any read property next mutates. onChange fires at most once per
        // installed tracker, so we install exactly one per refresh and
        // gate re-entry through `refreshScheduled` to keep the count flat.
        withObservationTracking {
            if let _ = state.activeIndex {
                button.title = " " + PomoCommand.formattedRemaining(state.secondsLeft)
                if button.image !== timerImage { button.image = timerImage }
            } else {
                button.title = ""
                if button.image != nil { button.image = nil }
            }
        } onChange: { [weak self] in
            DispatchQueue.main.async { self?.scheduleRefresh() }
        }
    }

    private func scheduleRefresh() {
        guard !refreshScheduled else { return }
        refreshScheduled = true
        refresh()
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
