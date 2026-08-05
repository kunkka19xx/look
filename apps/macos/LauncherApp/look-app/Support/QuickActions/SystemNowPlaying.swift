import Foundation
import OSLog

/// A snapshot of the current now-playing media, whatever app owns it.
struct NowPlayingSnapshot: Equatable {
    let title: String?
    let artist: String?
    let app: String?
    let isPlaying: Bool
    /// Serialized `MRPlayerPath` identifying the player (not the track), so it
    /// stays valid across track changes.
    let playerPath: Data?

    var hasTrack: Bool { title?.isEmpty == false }
}

/// Reads and controls system-wide now-playing via Apple's private MediaRemote
/// framework.
///
/// macOS 15.4+ lets only Apple-entitled binaries read now-playing, so the work
/// is delegated to `/usr/bin/osascript`, which is entitled. Not App Store safe,
/// and liable to break on a macOS update.
///
/// Commands must be addressed to a specific player, as the Linux/Windows app
/// does with an MPRIS bus name (`apps/linows/src-tauri/src/nowplaying.rs`). The
/// untargeted `MRMediaRemoteSendCommand` reports success but is answered by
/// Look's own `MPRemoteCommandCenter`, so it resumes Pomodoro music instead of
/// the tab on screen.
///
/// `nonisolated` because the target defaults to `MainActor` isolation, which
/// would make even closure literals here main-actor bound and trap the
/// concurrency runtime when reached from the background queue below.
nonisolated final class SystemNowPlaying: Sendable {
    static let shared = SystemNowPlaying()

    /// `MRMediaRemoteSendCommand` command codes.
    enum Command: Int32 {
        case togglePlayPause = 2
        case nextTrack = 4
        case previousTrack = 5
    }

    private static let mediaRemoteBundlePath =
        "/System/Library/PrivateFrameworks/MediaRemote.framework/"
    /// Kill a hung `osascript` so the poll never stalls.
    private static let scriptTimeout: TimeInterval = 4
    private static let commandPathEnvKey = "LOOK_NOWPLAYING_COMMAND_PATH"
    private static let commandCodeEnvKey = "LOOK_NOWPLAYING_COMMAND_CODE"
    private static let sendSuccessMarker = "ok"

    private let log = Logger(subsystem: "noah-code.Look", category: "nowplaying")

    private init() {}

    // MARK: Reading

    /// The current track, or nil when nothing is playing or the read fails.
    func current() async -> NowPlayingSnapshot? {
        await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(returning: self.read())
            }
        }
    }

    private func read() -> NowPlayingSnapshot? {
        guard let output = runScript(Self.readScript, environment: [:]),
              let result = try? JSONDecoder().decode(ReadResult.self, from: output),
              result.hasTrack else {
            return nil
        }
        return NowPlayingSnapshot(
            title: result.title,
            artist: result.artist,
            app: result.appName,
            isPlaying: result.isPlaying ?? false,
            playerPath: result.path.flatMap { Data(base64Encoded: $0) }
        )
    }

    // MARK: Sending

    /// Sends a transport command to the player `playerPath` identifies.
    @discardableResult
    func send(_ command: Command, to playerPath: Data?) async -> Bool {
        guard let playerPath else { return false }
        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(returning: self.sendTargeted(command, to: playerPath))
            }
        }
    }

    private func sendTargeted(_ command: Command, to playerPath: Data) -> Bool {
        let environment = [
            Self.commandPathEnvKey: playerPath.base64EncodedString(),
            Self.commandCodeEnvKey: String(command.rawValue),
        ]
        guard let output = runScript(Self.sendScript, environment: environment) else { return false }
        let text = String(decoding: output, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        return text == Self.sendSuccessMarker
    }

    // MARK: osascript plumbing

    private func runScript(_ script: String, environment: [String: String]) -> Data? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-l", "JavaScript", "-"]
        process.environment = ProcessInfo.processInfo.environment.merging(environment) { _, new in new }
        let input = Pipe()
        let output = Pipe()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            input.fileHandleForWriting.write(Data(script.utf8))
            input.fileHandleForWriting.closeFile()

            // Terminate on timeout so the pipe closes and the blocking read
            // below returns instead of hanging the caller.
            let terminator = DispatchWorkItem {
                if process.isRunning { process.terminate() }
            }
            DispatchQueue.global().asyncAfter(deadline: .now() + Self.scriptTimeout, execute: terminator)

            let data = output.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            terminator.cancel()
            guard process.terminationStatus == 0 else { return nil }
            return data
        } catch {
            log.error("now-playing script failed: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    // MARK: Decoding

    private struct ReadResult: Decodable {
        let title: String?
        let artist: String?
        let displayName: String?
        let isPlaying: Bool?
        /// Base64 `MRPlayerPath`.
        let path: String?

        var hasTrack: Bool { title?.isEmpty == false }

        var appName: String? { displayName?.isEmpty == false ? displayName : nil }
    }
}

// MARK: - Scripts

private nonisolated extension SystemNowPlaying {
    /// Reports the elected player as JSON.
    ///
    /// The track and the owning app come from two independent calls, and the
    /// system can hand the slot to another app in between, pairing one app's
    /// track with another's name. The path is read on both sides and the result
    /// discarded if it moved.
    static let readScript = """
    function run() {
        try {
            const bundle = $.NSBundle.bundleWithPath('\(mediaRemoteBundlePath)');
            bundle.load;
            const Req = $.NSClassFromString('MRNowPlayingRequest');
            if (!Req) return JSON.stringify({});

            const unwrap = (v) => (v && !v.isNil()) ? ObjC.unwrap(v) : null;
            const pathData = () => {
                const p = Req.localNowPlayingPlayerPath;
                if (!p || p.isNil()) return null;
                try { return unwrap(p.data.base64EncodedStringWithOptions(0)); } catch (e) { return null; }
            };

            const before = pathData();
            const item = Req.localNowPlayingItem;
            if (!item || item.isNil()) return JSON.stringify({});
            const info = item.nowPlayingInfo;
            if (!info || info.isNil()) return JSON.stringify({});
            const after = pathData();
            if (before !== after) return JSON.stringify({});

            const get = (k) => {
                const v = info.valueForKey(k);
                return (v && !v.isNil()) ? ObjC.deepUnwrap(v) : null;
            };
            const out = {
                title: get('kMRMediaRemoteNowPlayingInfoTitle'),
                artist: get('kMRMediaRemoteNowPlayingInfoArtist'),
                isPlaying: false,
                path: after
            };
            const rate = get('kMRMediaRemoteNowPlayingInfoPlaybackRate');
            if (typeof rate === 'number') out.isPlaying = rate > 0;

            const path = Req.localNowPlayingPlayerPath;
            if (path && !path.isNil()) {
                try {
                    const client = path.client;
                    if (client && !client.isNil()) {
                        out.displayName = unwrap(client.displayName);
                    }
                } catch (e) {}
            }
            return JSON.stringify(out);
        } catch (e) {
            return JSON.stringify({});
        }
    }
    """

    /// Rebuilds a player path and sends one transport command to it.
    static let sendScript = """
    function run() {
        const env = $.NSProcessInfo.processInfo.environment;
        const envValue = (key) => {
            const v = env.objectForKey(key);
            return (v && !v.isNil()) ? ObjC.unwrap(v) : null;
        };
        try {
            const bundle = $.NSBundle.bundleWithPath('\(mediaRemoteBundlePath)');
            bundle.load;
            const encoded = envValue('\(commandPathEnvKey)');
            const code = parseInt(envValue('\(commandCodeEnvKey)'), 10);
            if (!encoded || isNaN(code)) return '';

            const raw = $.NSData.alloc.initWithBase64EncodedStringOptions(encoded, 0);
            const PathClass = $.NSClassFromString('MRPlayerPath');
            const path = PathClass.alloc.initWithData(raw);
            if (!path || path.isNil()) return '';

            const Req = $.NSClassFromString('MRNowPlayingRequest');
            const request = Req.alloc.initWithPlayerPath(path);
            if (!request || request.isNil()) return '';

            request.sendCommandOptionsQueueCompletion(code, $(), $(), $());
            return '\(sendSuccessMarker)';
        } catch (e) {
            return '';
        }
    }
    """
}
