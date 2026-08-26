import AppKit
import Combine
import Foundation

/// A tick source for launchpad tiles that runs only while Look is active.
///
/// The launcher window is ordered out on hide but its SwiftUI tree stays
/// mounted, so a plain `Timer.publish` keeps waking the main thread while
/// Look idles in the background. Activation tracks visibility: every hide
/// path either reactivates the previous app or reacts to `didResignActive`,
/// so resigning active is the moment the launcher left the screen.
///
/// `tick` fires once immediately on each show (so a clock repaints with the
/// current time instead of the time it froze at on hide), then every
/// `interval` while active. Held via `@StateObject` so the timer survives
/// view rebuilds and dies with the tile.
final class LauncherActiveTicker: ObservableObject {
    let tick = PassthroughSubject<Date, Never>()

    private let interval: TimeInterval
    private var timer: AnyCancellable?
    private var activationObservers: [AnyCancellable] = []

    init(every interval: TimeInterval) {
        self.interval = interval
        let center = NotificationCenter.default
        activationObservers = [
            center.publisher(for: NSApplication.didBecomeActiveNotification)
                .sink { [weak self] _ in self?.start() },
            center.publisher(for: NSApplication.didResignActiveNotification)
                .sink { [weak self] _ in self?.stop() },
        ]
        if NSApplication.shared.isActive {
            start()
        }
    }

    private func start() {
        guard timer == nil else { return }
        tick.send(Date())
        timer = Timer.publish(every: interval, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] date in self?.tick.send(date) }
    }

    private func stop() {
        timer = nil
    }
}
