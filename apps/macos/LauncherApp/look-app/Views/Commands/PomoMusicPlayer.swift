import AVFoundation
import Foundation
import Observation

// Streams one audio file at a time via AVPlayer. Track URLs are stored
// as paths (lightweight); only the currently-playing AVPlayerItem holds
// a buffer in memory. On track end we auto-advance, looping back to the
// first when the list wraps. Top-level folder scan only - no recursion.

@Observable
final class PomoMusicPlayer {
    private(set) var tracks: [URL] = []
    private(set) var currentIndex: Int?
    private(set) var isPlaying = false
    private(set) var folderPath: String?

    @ObservationIgnored nonisolated(unsafe) private var player: AVPlayer?
    @ObservationIgnored nonisolated(unsafe) private var endObserver: NSObjectProtocol?

    static let supportedExtensions: Set<String> = [
        "mp3", "m4a", "wav", "aac", "flac", "ogg", "aiff", "alac",
    ]

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
        // Inline cleanup so deinit stays nonisolated. Pause + observer
        // removal are safe to call from any thread.
        player?.pause()
        if let endObserver {
            NotificationCenter.default.removeObserver(endObserver)
        }
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
