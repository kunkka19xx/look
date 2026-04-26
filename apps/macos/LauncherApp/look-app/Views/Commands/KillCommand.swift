import AppKit
import SwiftUI

struct KillCommand {
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

    private static func parsePort(from searchTerm: String) -> Int? {
        let trimmed = searchTerm.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix(":") {
            return Int(trimmed.dropFirst())
        }
        let lower = trimmed.lowercased()
        if lower.hasPrefix("port ") {
            return Int(lower.dropFirst(5).trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return nil
    }

    private static func isPortQuery(_ searchTerm: String) -> Bool {
        let trimmed = searchTerm.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return trimmed.hasPrefix(":") || trimmed.hasPrefix("port ")
    }

    private static func commandName(for pid: Int32) -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/ps")
        process.arguments = ["-p", "\(pid)", "-o", "comm="]
        let outputPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = Pipe()

        do {
            try process.run()
            process.waitUntilExit()
            let data = outputPipe.fileHandleForReading.readDataToEndOfFile()
            let value = String(data: data, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            return (value?.isEmpty == false) ? value! : "Process \(pid)"
        } catch {
            return "Process \(pid)"
        }
    }

    private static func processCandidates(onPort port: Int) -> [Candidate] {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/sbin/lsof")
        process.arguments = ["-nP", "-iTCP:\(port)", "-sTCP:LISTEN", "-t"]
        let outputPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = Pipe()

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return []
        }

        let data = outputPipe.fileHandleForReading.readDataToEndOfFile()
        guard let output = String(data: data, encoding: .utf8) else { return [] }
        let pids = output
            .split(separator: "\n")
            .compactMap { Int32($0.trimmingCharacters(in: .whitespacesAndNewlines)) }

        var seen = Set<Int32>()
        let uniquePIDs = pids.filter { seen.insert($0).inserted }

        return uniquePIDs.enumerated().map { index, pid in
            let name = commandName(for: pid)
            return Candidate(
                id: "port-\(port)-\(pid)-\(index)",
                displayName: name,
                pid: pid,
                icon: NSWorkspace.shared.icon(forFile: name),
                number: index + 1,
                detail: "PID: \(pid)  •  Port: \(port)"
            )
        }
    }

    static func suggestions(searchTerm: String) -> [Candidate] {
        if isPortQuery(searchTerm) {
            guard let port = parsePort(from: searchTerm), (1...65_535).contains(port) else {
                return []
            }
            return processCandidates(onPort: port)
        }

        let apps = getRunningApps()
        let candidates = appCandidates(from: apps)
        if searchTerm.isEmpty {
            return candidates
        }

        if let num = Int(searchTerm), num > 0 && num <= candidates.count {
            return [candidates[num - 1]]
        }

        return candidates.filter { $0.displayName.lowercased().contains(searchTerm.lowercased()) }
    }

    static func kill(pid: Int32, name: String, completion: @escaping (String) -> Void) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/kill")
        process.arguments = ["-9", "\(pid)"]

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                completion("Killed: \(name) (PID: \(pid))")
            } else {
                completion("Failed to kill \(name): permission denied")
            }
        } catch {
            completion("Error: \(error.localizedDescription)")
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
                            Image(nsImage: candidate.icon ?? NSWorkspace.shared.icon(forFileType: "public.application"))
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
            Image(nsImage: candidate.icon ?? NSWorkspace.shared.icon(forFileType: "public.application"))
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
