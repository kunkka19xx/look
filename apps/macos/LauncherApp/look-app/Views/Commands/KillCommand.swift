import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct KillCommand {
    /// Row cap for the fuzzy query path (mirrors linows `KILL_RESULT_LIMIT`).
    private static let resultLimit = 60

    struct Candidate: Identifiable {
        let id: String
        let displayName: String
        let pid: Int32
        let icon: NSImage?
        let number: Int
        let detail: String
    }

    private static func getRunningApps() -> [NSRunningApplication] {
        NSWorkspace.shared.runningApplications
            .filter { $0.activationPolicy == .regular }
            .sorted { ($0.localizedName ?? "") < ($1.localizedName ?? "") }
    }

    private static func appCandidates(from apps: [NSRunningApplication]) -> [Candidate] {
        apps.enumerated().map { index, app in
            Candidate(
                id: "app-\(app.processIdentifier)-\(index)",
                displayName: app.localizedName ?? "Unknown",
                pid: app.processIdentifier,
                icon: app.icon,
                number: index + 1,
                detail: "PID: \(app.processIdentifier)"
            )
        }
    }

    /// Strips the optional `:` / `port ` port-search affordance so `:3000` and
    /// `3000` resolve identically through the one numeric path (exact port >
    /// partial > PID) rather than a separate lsof lookup.
    private static func normalize(_ searchTerm: String) -> String {
        let trimmed = searchTerm.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix(":") {
            return String(trimmed.dropFirst()).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        if trimmed.lowercased().hasPrefix("port ") {
            return String(trimmed.dropFirst(5)).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return trimmed
    }

    static func suggestions(searchTerm: String) -> [Candidate] {
        let term = normalize(searchTerm)
        let apps = getRunningApps()
        // Empty query lists apps only (the panel's default view).
        if term.isEmpty {
            return appCandidates(from: apps)
        }

        // Non-empty query: fuzzy over apps + processes, apps first, deduped by
        // PID (ProcessScoring mirrors the Rust `rank_kill_targets`; the fuzzy
        // score is the same `core/matching` scorer over FFI).
        let appPairs = apps.map { (name: $0.localizedName ?? "Unknown", pid: $0.processIdentifier) }
        let procs = ProcessService.enumerate().map {
            ProcessScoring.Candidate(name: $0.name, pid: $0.pid, ports: $0.ports)
        }
        let lowered = term.lowercased()
        let ranked = ProcessScoring.rankKillTargets(
            apps: appPairs, procs: procs, query: term
        ) { title in
            EngineBridge.shared.fuzzyScore(query: lowered, title: title)
        }

        return ranked.prefix(resultLimit).enumerated().map { index, target in
            Candidate(
                id: "\(target.isApp ? "app" : "proc")-\(target.pid)-\(index)",
                displayName: target.name,
                pid: target.pid,
                icon: LauncherProcessFeature.icon(forPID: target.pid),
                number: index + 1,
                detail: "PID: \(target.pid)"
            )
        }
    }

    /// SIGKILL a process, routed through the native `ProcessService.kill`.
    static func kill(pid: Int32, name: String, completion: @escaping (String) -> Void) {
        switch ProcessService.kill(pid: pid) {
        case .success:
            completion("Killed: \(name) (PID: \(pid))")
        case .failure(let error):
            completion("Failed to kill \(name): \(error.message)")
        }
    }
}

struct KillCommandView: View {
    let suggestions: [KillCommand.Candidate]
    let selectedIndex: Int?
    let emptyMessage: String
    let themeStore: ThemeStore

    let onSelect: (KillCommand.Candidate) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(suggestions.prefix(20), id: \.id) { candidate in
                    Button {
                        onSelect(candidate)
                    } label: {
                        HStack(spacing: 10) {
                            Image(nsImage: candidate.icon ?? NSWorkspace.shared.icon(for: .application))
                                .resizable()
                                .frame(width: 20, height: 20)
                            Text(candidate.displayName)
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                                .foregroundStyle(themeStore.fontColor())
                            Spacer()
                            Text(candidate.detail)
                                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                            if selectedIndex == candidate.number {
                                Text("→ Enter")
                                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                                    .foregroundStyle(themeStore.accentColor())
                            }
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(
                            selectedIndex == candidate.number
                                ? themeStore.selectionFillColor() : .clear,
                            in: RoundedRectangle(cornerRadius: 6, style: .continuous)
                        )
                    }
                    .buttonStyle(.plain)
                }

                if suggestions.isEmpty {
                    Text(emptyMessage)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                }
            }
            .padding(2)
        }
    }
}

struct KillConfirmationBar: View {
    let candidate: KillCommand.Candidate
    let themeStore: ThemeStore
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(nsImage: candidate.icon ?? NSWorkspace.shared.icon(for: .application))
                .resizable()
                .frame(width: 24, height: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text("Kill \(candidate.displayName)?")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                Text(candidate.detail)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            }
            Spacer()
            Button {
                onConfirm()
            } label: {
                Text("Y / Yes")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                    .foregroundStyle(themeStore.onDangerColor())
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(themeStore.dangerColor(), in: Capsule())
            }
            .buttonStyle(.plain)
            Button {
                onCancel()
            } label: {
                Text("N / No")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                    .foregroundStyle(themeStore.fontColor())
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(themeStore.controlFillColor(), in: Capsule())
            }
            .buttonStyle(.plain)
        }
        .padding(10)
        .background(themeStore.controlFillColor().opacity(0.92), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}
