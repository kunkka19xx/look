import Foundation
import AppKit
import OSLog
@preconcurrency import UserNotifications

// nonisolated so the UNUserNotificationCenter completion-queue
// closures can write to it without needing a MainActor hop.
nonisolated private let pomoNotifLog = Logger(subsystem: "noah-code.Look", category: "pomo-notif")

// ── Model ──────────────────────────────────────────────────────────────

struct PomoSession: Identifiable, Equatable {
    let id: UUID
    var type: SessionType
    var durationMinutes: Int
    var name: String

    enum SessionType: String, Equatable {
        case focus
        case `break`
    }

    nonisolated init(id: UUID = UUID(), type: SessionType, durationMinutes: Int, name: String) {
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
    static let idleFadeSeconds: TimeInterval = 5
    static let menuBarTickSeconds: TimeInterval = 1.0
    static let endingSoonThresholdSeconds = 10

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
    // Bool flag - accessed from the UNUserNotificationCenter completion
    // queue (not main). nonisolated(unsafe) is fine: occasional
    // double-set is harmless (worst case: we issue the auth prompt
    // twice; the system de-dupes).
    nonisolated(unsafe) private static var permissionRequested = false

    static func notifyEndingSoon(session: PomoSession, secondsLeft: Int) {
        pomoNotifLog.notice("notifyEndingSoon name=\(session.name, privacy: .public) secondsLeft=\(secondsLeft)")
        NSSound(named: "Tink")?.play()
        let title = session.type == .focus ? "Focus ending soon" : "Break ending soon"
        let subtitle = "\(session.name) - \(secondsLeft)s left"
        NotificationCenter.default.post(
            name: .lookPomoStatusMessage,
            object: nil,
            userInfo: ["title": title, "subtitle": subtitle]
        )
        ensurePermission { granted in
            guard granted else { return }
            let content = UNMutableNotificationContent()
            content.title = session.type == .focus ? "Focus ending soon" : "Break ending soon"
            content.body = "\(session.name) - \(secondsLeft)s left"
            content.sound = .default
            content.interruptionLevel = .timeSensitive
            let req = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
            deliver(req, label: "endingSoon")
        }
    }

    static func notifyPhaseTransition(finished: PomoSession, next: PomoSession?) {
        pomoNotifLog.notice("notifyPhaseTransition finished=\(finished.name, privacy: .public) hasNext=\(next != nil)")
        NSSound(named: "Glass")?.play()
        let title = finished.type == .focus ? "Focus done" : "Break done"
        let subtitle: String
        if let next {
            subtitle = "Next: \(next.name) (\(next.durationMinutes) min)"
        } else {
            subtitle = "All sessions complete"
        }
        NotificationCenter.default.post(
            name: .lookPomoStatusMessage,
            object: nil,
            userInfo: ["title": title, "subtitle": subtitle]
        )
        ensurePermission { granted in
            guard granted else { return }
            let content = UNMutableNotificationContent()
            content.title = finished.type == .focus ? "Focus done" : "Break done"
            if let next {
                content.body = "Next: \(next.name) (\(next.durationMinutes) min)"
            } else {
                content.body = "All sessions complete"
            }
            content.sound = .default
            content.interruptionLevel = .timeSensitive
            let req = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
            deliver(req, label: "phaseTransition")
        }
    }

    // Request notification permission proactively at app launch so the
    // first phase transition isn't where the user discovers the
    // permission flow.
    static func requestPermissionEarly() {
        ensurePermission { _ in /* answer comes later, just prime it */ }
    }

    // Forward foreground notifications so they show even while the
    // launcher window is active. Set this delegate at app launch.
    final class ForegroundDeliveryDelegate: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
        func userNotificationCenter(
            _ center: UNUserNotificationCenter,
            willPresent notification: UNNotification,
            withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
        ) {
            completionHandler([.banner, .sound, .list])
        }
    }
    static let foregroundDelegate = ForegroundDeliveryDelegate()

    private static func ensurePermission(_ then: @escaping @Sendable (Bool) -> Void) {
        let center = UNUserNotificationCenter.current()
        let bundleID = Bundle.main.bundleIdentifier ?? "<no-bundle-id>"
        center.getNotificationSettings { settings in
            let statusName: String
            switch settings.authorizationStatus {
            case .authorized: statusName = "authorized"
            case .denied: statusName = "denied"
            case .notDetermined: statusName = "notDetermined"
            case .provisional: statusName = "provisional"
            case .ephemeral: statusName = "ephemeral"
            @unknown default: statusName = "unknown"
            }
            pomoNotifLog.notice("auth status=\(statusName, privacy: .public) bundle=\(bundleID, privacy: .public) alertSetting=\(settings.alertSetting.rawValue) soundSetting=\(settings.soundSetting.rawValue)")
            switch settings.authorizationStatus {
            case .authorized, .provisional, .ephemeral:
                then(true)
            case .denied:
                pomoNotifLog.error("DENIED - open System Settings → Notifications → \(bundleID, privacy: .public) and enable")
                then(false)
            case .notDetermined:
                guard !permissionRequested else { then(false); return }
                permissionRequested = true
                pomoNotifLog.notice("requesting authorization …")
                center.requestAuthorization(options: [.alert, .sound]) { granted, error in
                    if let error {
                        pomoNotifLog.error("requestAuthorization error: \(error.localizedDescription, privacy: .public)")
                    }
                    pomoNotifLog.notice("requestAuthorization granted=\(granted)")
                    then(granted)
                }
            @unknown default:
                then(false)
            }
        }
    }

    // Wrap center.add so we capture any post-add error (e.g. malformed
    // request, system unavailable). Without this completion handler the
    // failure is silent.
    nonisolated private static func deliver(_ req: UNNotificationRequest, label: String) {
        UNUserNotificationCenter.current().add(req) { error in
            if let error {
                pomoNotifLog.error("\(label, privacy: .public) add error: \(error.localizedDescription, privacy: .public)")
            } else {
                pomoNotifLog.notice("\(label, privacy: .public) added id=\(req.identifier, privacy: .public)")
            }
        }
    }
}
