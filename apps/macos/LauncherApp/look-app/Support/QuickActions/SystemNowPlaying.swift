import Foundation
import OSLog

/// A snapshot of the system-wide "now playing" media, whatever app owns it
/// (a browser tab, Spotify, Apple Music, Look's own Pomodoro player, ...).
struct NowPlayingSnapshot: Equatable {
    let title: String?
    let artist: String?
    /// The owning app's display name, e.g. "Firefox", "Spotify".
    let app: String?
    let isPlaying: Bool

    var hasTrack: Bool { title?.isEmpty == false }
}

/// Reads and controls the system-wide now-playing media via Apple's private
/// MediaRemote framework.
///
/// macOS 15.4+ blocks a normal app from reading now-playing directly (the
/// `mediaremoted` daemon checks for an Apple-only entitlement), so the READ is
/// delegated to `/usr/bin/osascript`, a system binary that *is* entitled; its
/// bundled JXA loads MediaRemote and prints the track as JSON. Sending transport
/// commands was never gated, so that path calls `MRMediaRemoteSendCommand`
/// directly. This is a private-API path (not App Store safe) and could break on
/// a future macOS update; it is used only for this read-only glance + transport.
final class SystemNowPlaying: Sendable {
    static let shared = SystemNowPlaying()

    /// `MRMediaRemoteSendCommand` command codes (stable for years).
    enum Command: Int32 {
        case togglePlayPause = 2
        case nextTrack = 4
        case previousTrack = 5
    }

    private static let mediaRemotePath =
        "/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote"

    private typealias SendCommand = @convention(c) (Int32, CFDictionary?) -> Bool
    private let sendCommand: SendCommand?

    private let log = Logger(subsystem: "noah-code.Look", category: "nowplaying")

    private init() {
        if let handle = dlopen(Self.mediaRemotePath, RTLD_NOW),
           let symbol = dlsym(handle, "MRMediaRemoteSendCommand") {
            sendCommand = unsafeBitCast(symbol, to: SendCommand.self)
        } else {
            sendCommand = nil
        }
    }

    /// Sends a transport command to whichever app owns now-playing.
    @discardableResult
    func send(_ command: Command) -> Bool {
        sendCommand?(command.rawValue, nil) ?? false
    }

    /// Reads the current now-playing track, or nil when nothing is playing or the
    /// read fails. Runs `osascript` off the main thread.
    func current() async -> NowPlayingSnapshot? {
        await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(returning: self.readViaOSAScript())
            }
        }
    }

    private func readViaOSAScript() -> NowPlayingSnapshot? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-l", "JavaScript", "-"]
        let input = Pipe()
        let output = Pipe()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            input.fileHandleForWriting.write(Data(Self.readScript.utf8))
            input.fileHandleForWriting.closeFile()
            let data = output.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            return Self.parse(data)
        } catch {
            log.error("now-playing read failed: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    private static func parse(_ data: Data) -> NowPlayingSnapshot? {
        guard let result = try? JSONDecoder().decode(ReadResult.self, from: data) else { return nil }
        guard result.track == true, let title = result.title, !title.isEmpty else { return nil }
        return NowPlayingSnapshot(
            title: title,
            artist: result.artist,
            app: result.app,
            isPlaying: result.isPlaying ?? false
        )
    }

    private struct ReadResult: Decodable {
        let track: Bool?
        let title: String?
        let artist: String?
        let app: String?
        let isPlaying: Bool?
    }

    /// JXA run under `osascript` (an Apple-entitled binary): loads MediaRemote,
    /// reads `MRNowPlayingRequest`, and prints the track as JSON. Kept minimal and
    /// defensive so any failure yields parseable output rather than a crash.
    private static let readScript = """
    function run() {
        try {
            const MediaRemote = $.NSBundle.bundleWithPath('/System/Library/PrivateFrameworks/MediaRemote.framework/');
            MediaRemote.load;
            const Req = $.NSClassFromString('MRNowPlayingRequest');
            if (!Req) return JSON.stringify({ track: false });
            const item = Req.localNowPlayingItem;
            if (!item || item.isNil()) return JSON.stringify({ track: false });
            const info = item.nowPlayingInfo;
            const get = (k) => {
                const v = info.valueForKey(k);
                return (v && !v.isNil()) ? ObjC.deepUnwrap(v) : null;
            };
            let app = null;
            try {
                const client = Req.localNowPlayingPlayerPath.client;
                if (client && !client.isNil()) app = ObjC.unwrap(client.displayName);
            } catch (e) {}
            const rate = get('kMRMediaRemoteNowPlayingInfoPlaybackRate');
            return JSON.stringify({
                track: true,
                title: get('kMRMediaRemoteNowPlayingInfoTitle'),
                artist: get('kMRMediaRemoteNowPlayingInfoArtist'),
                app: app,
                isPlaying: (typeof rate === 'number') ? rate > 0 : false
            });
        } catch (e) {
            return JSON.stringify({ track: false });
        }
    }
    """
}
