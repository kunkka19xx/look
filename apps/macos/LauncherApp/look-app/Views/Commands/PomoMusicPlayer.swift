import AVFoundation
import Foundation
import MediaPlayer
import Observation

// Streams one audio file at a time via AVPlayer. Track URLs are stored
// as paths (lightweight); only the currently-playing AVPlayerItem holds
// a buffer in memory. On track end we auto-advance, looping back to the
// first when the list wraps. Top-level folder scan only - no recursion.
//
// Playback is published to the system Now Playing (Control Center, menu-bar
// media widget, the hardware media keys) via MediaPlayer, so Look's focus
// music behaves like any other media app. That is the publish direction and
// uses public API, unlike reading other apps' now-playing, which macOS blocks.

@Observable
final class PomoMusicPlayer {
    private(set) var tracks: [URL] = []
    private(set) var currentIndex: Int?
    private(set) var isPlaying = false
    private(set) var folderPath: String?

    @ObservationIgnored nonisolated(unsafe) private var player: AVPlayer?
    @ObservationIgnored nonisolated(unsafe) private var endObserver: NSObjectProtocol?
    /// Remote-command handlers registered on the process-wide command center,
    /// kept so `deinit` can remove them and never leave stale targets behind.
    @ObservationIgnored nonisolated(unsafe) private var commandTargets: [(command: MPRemoteCommand, token: Any)] = []

    static let supportedExtensions: Set<String> = [
        "mp3", "m4a", "wav", "aac", "flac", "ogg", "aiff", "alac",
    ]

    init() {
        configureRemoteCommands()
    }

    var currentTitle: String? {
        guard let i = currentIndex, tracks.indices.contains(i) else { return nil }
        return tracks[i].deletingPathExtension().lastPathComponent
    }

    var hasFolder: Bool { folderPath != nil }

    // Pick a new folder. Re-scans + shuffles. Stops anything playing.
    func setFolder(_ url: URL) {
        clearPlayer()
        folderPath = url.path
        tracks = scanFolder(url).shuffled()
        currentIndex = nil
        isPlaying = false
        clearNowPlayingInfo()
    }

    // Re-establish a previously-saved folder on app launch. Same as
    // setFolder but skips silently if the path is gone.
    func restore(folderPath: String?) {
        guard let path = folderPath, !path.isEmpty else { return }
        let url = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        setFolder(url)
    }

    func clearFolder() {
        clearPlayer()
        tracks = []
        currentIndex = nil
        folderPath = nil
        isPlaying = false
        clearNowPlayingInfo()
    }

    func togglePlay() {
        if currentIndex == nil {
            // Cold start - kick off from the first shuffled track.
            guard !tracks.isEmpty else { return }
            loadAndPlay(index: 0)
            return
        }
        if isPlaying {
            player?.pause()
            isPlaying = false
        } else {
            player?.play()
            isPlaying = true
        }
        updateNowPlayingInfo()
    }

    func next() {
        guard !tracks.isEmpty else { return }
        let target: Int
        if let i = currentIndex {
            target = (i + 1) % tracks.count
        } else {
            target = 0
        }
        loadAndPlay(index: target)
    }

    func prev() {
        guard !tracks.isEmpty else { return }
        let target: Int
        if let i = currentIndex {
            target = (i - 1 + tracks.count) % tracks.count
        } else {
            target = 0
        }
        loadAndPlay(index: target)
    }

    private func loadAndPlay(index: Int) {
        guard tracks.indices.contains(index) else { return }
        clearPlayer()
        let url = tracks[index]
        let item = AVPlayerItem(url: url)
        let newPlayer = AVPlayer(playerItem: item)
        endObserver = NotificationCenter.default.addObserver(
            forName: AVPlayerItem.didPlayToEndTimeNotification,
            object: item,
            queue: .main
        ) { [weak self] _ in
            // Notification queue: .main delivers on the main thread; the
            // explicit hop satisfies Swift 6's @Sendable-closure check.
            MainActor.assumeIsolated {
                // Loops at end of list (next wraps via modulo).
                self?.next()
            }
        }
        player = newPlayer
        currentIndex = index
        isPlaying = true
        newPlayer.play()
        updateNowPlayingInfo()
    }

    private func clearPlayer() {
        player?.pause()
        if let endObserver {
            NotificationCenter.default.removeObserver(endObserver)
        }
        endObserver = nil
        player = nil
    }

    deinit {
        // Inline cleanup so deinit stays nonisolated. Pause + observer/target
        // removal are safe to call from any thread.
        player?.pause()
        if let endObserver {
            NotificationCenter.default.removeObserver(endObserver)
        }
        for entry in commandTargets {
            entry.command.removeTarget(entry.token)
        }
    }

    // ── System Now Playing ────────────────────────────────────────────────

    // Register the hardware media keys / Control Center transport so they drive
    // this player. MediaPlayer delivers these handlers on the main thread, hence
    // the assumeIsolated hop to touch the observable state (same pattern as the
    // track-end observer above).
    private func configureRemoteCommands() {
        let center = MPRemoteCommandCenter.shared()
        register(center.playCommand) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, !self.isPlaying else { return }
                self.togglePlay()
            }
            return .success
        }
        register(center.pauseCommand) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.isPlaying else { return }
                self.togglePlay()
            }
            return .success
        }
        register(center.togglePlayPauseCommand) { [weak self] _ in
            MainActor.assumeIsolated { self?.togglePlay() }
            return .success
        }
        register(center.nextTrackCommand) { [weak self] _ in
            MainActor.assumeIsolated { self?.next() }
            return .success
        }
        register(center.previousTrackCommand) { [weak self] _ in
            MainActor.assumeIsolated { self?.prev() }
            return .success
        }
    }

    /// Adds a handler and keeps its token so `deinit` can remove it.
    private func register(
        _ command: MPRemoteCommand,
        handler: @escaping (MPRemoteCommandEvent) -> MPRemoteCommandHandlerStatus
    ) {
        commandTargets.append((command, command.addTarget(handler: handler)))
    }

    // Publish the current track and play state to the system. The system
    // extrapolates the progress bar from the elapsed time and playback rate, so
    // updating on each track change and play/pause is enough.
    private func updateNowPlayingInfo() {
        guard let title = currentTitle else {
            clearNowPlayingInfo()
            return
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: title,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
            MPNowPlayingInfoPropertyPlaybackRate: isPlaying ? 1.0 : 0.0,
        ]
        if let item = player?.currentItem {
            let duration = item.duration.seconds
            if duration.isFinite, duration > 0 {
                info[MPMediaItemPropertyPlaybackDuration] = duration
            }
            let elapsed = player?.currentTime().seconds ?? 0
            if elapsed.isFinite {
                info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = elapsed
            }
        }
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = info
        center.playbackState = isPlaying ? .playing : .paused
    }

    private func clearNowPlayingInfo() {
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = nil
        center.playbackState = .stopped
    }

    private func scanFolder(_ url: URL) -> [URL] {
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: url,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }
        return entries.filter { Self.supportedExtensions.contains($0.pathExtension.lowercased()) }
    }
}
