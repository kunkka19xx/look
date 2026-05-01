import Foundation
import AppKit
@preconcurrency import UserNotifications

// ── Model ──────────────────────────────────────────────────────────────

nonisolated struct PomoSession: Identifiable, Equatable {
    let id: UUID
    var type: SessionType
    var durationMinutes: Int
    var name: String

    enum SessionType: String, Equatable {
        case focus
        case `break`
    }

    init(id: UUID = UUID(), type: SessionType, durationMinutes: Int, name: String) {
        self.id = id
        self.type = type
        self.durationMinutes = durationMinutes
        self.name = name
    }
}

enum PomoTimerStyle: String, CaseIterable, Identifiable {
    case modern, vintage, minimal

    var id: String { rawValue }
    var title: String {
        switch self {
        case .modern: return "Modern Ring"
        case .vintage: return "Vintage Dial"
        case .minimal: return "Minimal Text"
        }
    }
}

enum PomoCommand {
    static func defaultSessions() -> [PomoSession] {
        [
            PomoSession(type: .focus, durationMinutes: 30, name: "Deep Work"),
            PomoSession(type: .break, durationMinutes: 5, name: "Short Break"),
            PomoSession(type: .focus, durationMinutes: 30, name: "Review"),
            PomoSession(type: .break, durationMinutes: 5, name: "Short Break"),
            PomoSession(type: .focus, durationMinutes: 30, name: "Wrap Up"),
            PomoSession(type: .break, durationMinutes: 15, name: "Long Break"),
        ]
    }

    static let focusDefaultMinutes = 30
    static let breakDefaultMinutes = 5
    static let idleFadeSeconds: TimeInterval = 15
    static let menuBarTickSeconds: TimeInterval = 1.0

    static func formattedRemaining(_ seconds: Int) -> String {
        let safe = max(0, seconds)
        let m = safe / 60
        let s = safe % 60
        return String(format: "%02d:%02d", m, s)
    }

    static func formattedTotal(_ seconds: Int) -> String {
        let safe = max(0, seconds)
        let h = safe / 3600
        let m = (safe % 3600) / 60
        return h > 0 ? "\(h)h \(m)m" : "\(m)m"
    }
}

// ── Persistence: read/write pomo_* keys in .look.config ────────────────
//
// Keys are all optional. Missing keys fall back to defaults so users who
// never touch /pomo aren't affected by its existence.

enum PomoPersistence {
    private static let sessionsKey = "pomo_sessions"
    private static let timerStyleKey = "pomo_timer_style"
    private static let musicFolderKey = "pomo_music_folder"

    struct Snapshot {
        var sessions: [PomoSession]
        var timerStyle: PomoTimerStyle
        var musicFolderPath: String?
    }

    static func load() -> Snapshot {
        let path = ConfigPathResolver.resolvedPath()
        guard let raw = try? String(contentsOfFile: path, encoding: .utf8) else {
            return Snapshot(sessions: PomoCommand.defaultSessions(), timerStyle: .modern, musicFolderPath: nil)
        }
        let kv = parseKeyValues(raw)
        let sessions = kv[sessionsKey].flatMap(decodeSessions) ?? PomoCommand.defaultSessions()
        let style = kv[timerStyleKey].flatMap(PomoTimerStyle.init(rawValue:)) ?? .modern
        let folder = kv[musicFolderKey]?.trimmingCharacters(in: .whitespacesAndNewlines)
        return Snapshot(
            sessions: sessions,
            timerStyle: style,
            musicFolderPath: (folder?.isEmpty == false) ? folder : nil
        )
    }

    static func save(_ snapshot: Snapshot) {
        let path = ConfigPathResolver.resolvedPath()
        var lines: [String] = []
        if let raw = try? String(contentsOfFile: path, encoding: .utf8) {
            lines = raw.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline).map(String.init)
        }
        upsert(&lines, key: sessionsKey, value: encodeSessions(snapshot.sessions))
        upsert(&lines, key: timerStyleKey, value: snapshot.timerStyle.rawValue)
        if let folder = snapshot.musicFolderPath, !folder.isEmpty {
            upsert(&lines, key: musicFolderKey, value: folder)
        } else {
            remove(&lines, key: musicFolderKey)
        }
        let payload = lines.joined(separator: "\n") + "\n"
        try? payload.write(toFile: path, atomically: true, encoding: .utf8)
    }

    // ── Encoding ────────────────────────────────────────────────────────
    // Each session: type:durationMin:name (name URL-encoded so commas/colons survive).
    // Sessions are joined with `,`.

    private static func encodeSessions(_ sessions: [PomoSession]) -> String {
        sessions.map { s in
            let type = s.type.rawValue
            let nameEncoded = s.name.addingPercentEncoding(withAllowedCharacters: .urlHostAllowed) ?? ""
            return "\(type):\(s.durationMinutes):\(nameEncoded)"
        }.joined(separator: ",")
    }

    nonisolated private static func decodeSessions(_ value: String) -> [PomoSession]? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        var result: [PomoSession] = []
        for token in trimmed.split(separator: ",") {
            let parts = token.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            guard parts.count == 3,
                  let type = PomoSession.SessionType(rawValue: String(parts[0])),
                  let mins = Int(parts[1]), mins > 0
            else { continue }
            let name = String(parts[2]).removingPercentEncoding ?? String(parts[2])
            result.append(PomoSession(type: type, durationMinutes: mins, name: name))
        }
        return result.isEmpty ? nil : result
    }

    // ── Lightweight config-line helpers ────────────────────────────────
    // Same `upsert`/`remove` pattern as ThemeStore but local to pomo so
    // we don't have to widen ThemeStore's private API.

    private static func parseKeyValues(_ raw: String) -> [String: String] {
        var out: [String: String] = [:]
        for line in raw.split(whereSeparator: \.isNewline) {
            let stripped = stripComment(String(line)).trimmingCharacters(in: .whitespacesAndNewlines)
            guard let eq = stripped.firstIndex(of: "=") else { continue }
            let key = String(stripped[..<eq]).trimmingCharacters(in: .whitespacesAndNewlines)
            let value = String(stripped[stripped.index(after: eq)...]).trimmingCharacters(in: .whitespacesAndNewlines)
            if !key.isEmpty {
                out[key] = value
            }
        }
        return out
    }

    private static func stripComment(_ line: String) -> String {
        guard let i = line.firstIndex(of: "#") else { return line }
        return String(line[..<i])
    }

    private static func upsert(_ lines: inout [String], key: String, value: String) {
        let prefix = "\(key)="
        for idx in lines.indices {
            let trimmed = stripComment(lines[idx]).trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.hasPrefix(prefix) {
                lines[idx] = "\(key)=\(value)"
                return
            }
        }
        lines.append("\(key)=\(value)")
    }

    private static func remove(_ lines: inout [String], key: String) {
        let prefix = "\(key)="
        lines.removeAll { line in
            stripComment(line).trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix(prefix)
        }
    }
}

// ── Notifications: phase-transition pings ─────────────────────────────
//
// Permission is requested lazily on the first phase transition. If the
// user denies, subsequent transitions silently no-op (the in-app banner
// still shows the transition).

enum PomoNotifications {
    private static var permissionRequested = false

    static func notifyPhaseTransition(finished: PomoSession, next: PomoSession?) {
        ensurePermission { granted in
            guard granted else { return }
            let center = UNUserNotificationCenter.current()
            let content = UNMutableNotificationContent()
            content.title = finished.type == .focus ? "Focus done" : "Break done"
            if let next {
                content.body = "Next: \(next.name) (\(next.durationMinutes) min)"
            } else {
                content.body = "All sessions complete"
            }
            content.sound = .default
            let req = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
            center.add(req)
        }
    }

    private static func ensurePermission(_ then: @escaping (Bool) -> Void) {
        let center = UNUserNotificationCenter.current()
        center.getNotificationSettings { settings in
            switch settings.authorizationStatus {
            case .authorized, .provisional, .ephemeral:
                then(true)
            case .denied:
                then(false)
            case .notDetermined:
                guard !permissionRequested else { then(false); return }
                permissionRequested = true
                center.requestAuthorization(options: [.alert, .sound]) { granted, _ in
                    then(granted)
                }
            @unknown default:
                then(false)
            }
        }
    }
}
