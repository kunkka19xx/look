import Foundation
import Observation

nonisolated enum SpeedTestDefaults {
    /// How recent a reading has to be for opening the panel to reuse it rather
    /// than spend bandwidth on a fresh run. The panel calls anything inside this
    /// window "just now", so the two read as one decision.
    static let autoRunFreshness: TimeInterval = 60
    /// The last reading, so the panel opens with a number instead of a blank.
    static let lastReadingKey = "look.speedtest.lastReading"
}

/// Drives the `/speed` panel: one run at a time, the last reading kept across
/// launches, and an elapsed counter while a run is in flight.
///
/// The controller outlives the panel, and a run outlives both: the native call
/// has no cancel, so closing `/speed` leaves it measuring and reopening rejoins
/// it in progress. Clearing the in-flight flag on close would let a second run
/// start alongside the first, and two measurements compete for the bandwidth
/// they are each trying to measure.
@MainActor
@Observable
final class SpeedTestController {
    private(set) var reading: SpeedReading?
    private(set) var isRunning = false
    private(set) var errorMessage: String?
    private(set) var elapsedSeconds = 0
    /// This machine's address on the LAN, refreshed with the panel.
    private(set) var localAddress: String?

    @ObservationIgnored private var tickTask: Task<Void, Never>?

    init() {
        reading = Self.loadLastReading()
    }

    private func refreshLocalAddress() {
        localAddress = LocalIPv4.current()
    }

    func start() {
        guard !isRunning else { return }
        isRunning = true
        errorMessage = nil
        elapsedSeconds = 0
        startTicking()

        // Resolved here, on the main actor: the shared instance is isolated, the
        // bridge itself is Sendable and does the work off it.
        let bridge = EngineBridge.shared
        Task { [weak self] in
            let outcome = await Task.detached(priority: .userInitiated) {
                bridge.speedTest()
            }.value

            guard let self else { return }
            self.finish(with: outcome)
        }
    }

    /// Runs unless the panel already has a recent reading, so tabbing back to
    /// `/speed` doesn't re-measure what it just measured.
    func startIfStale() {
        refreshLocalAddress()
        if let reading, Date().timeIntervalSince(reading.measuredAt) < SpeedTestDefaults.autoRunFreshness {
            return
        }
        start()
    }

    private func finish(with outcome: SpeedTestOutcome) {
        stopTicking()
        isRunning = false

        switch outcome {
        case let .reading(value):
            reading = value
            errorMessage = nil
            Self.saveLastReading(value)
        case let .failure(message):
            errorMessage = message
        }
    }

    private func startTicking() {
        stopTicking()
        tickTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, !Task.isCancelled else { return }
                self.elapsedSeconds += 1
            }
        }
    }

    private func stopTicking() {
        tickTask?.cancel()
        tickTask = nil
    }

    private static func loadLastReading() -> SpeedReading? {
        guard let data = UserDefaults.standard.data(forKey: SpeedTestDefaults.lastReadingKey) else {
            return nil
        }
        return try? JSONDecoder().decode(SpeedReading.self, from: data)
    }

    /// The public address is dropped on the way to disk: the panel masks it on
    /// screen, and a plist keeping it indefinitely is a stronger retention than
    /// that implies. A fresh run puts it back.
    private static func saveLastReading(_ value: SpeedReading) {
        var stored = value
        stored.publicIp = nil
        guard let data = try? JSONEncoder().encode(stored) else { return }
        UserDefaults.standard.set(data, forKey: SpeedTestDefaults.lastReadingKey)
    }
}
