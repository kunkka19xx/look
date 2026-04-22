import AppKit
import Combine
import SwiftUI

final class LauncherCommandService: ObservableObject {
    private let bridge: EngineBridge
    private let commandCatalog: [AppCommand]

    @Published var commandFeedback: String = ""
    @Published var pendingKillCandidate: KillCommand.Candidate?

    init(bridge: EngineBridge = .shared, commandCatalog: [AppCommand] = AppConstants.Launcher.commandCatalog) {
        self.bridge = bridge
        self.commandCatalog = commandCatalog
    }

    func resolveCommand(
        activeCommandID: String?,
        commandNamePart: String,
        selectedCommandID: String?
    ) -> AppCommand? {
        commandCatalog.first(where: { $0.id == (activeCommandID ?? "") })
            ?? commandCatalog.first(where: { $0.id == commandNamePart.lowercased() })
            ?? commandCatalog.first(where: { $0.id == selectedCommandID })
    }

    func runCommand(
        command: AppCommand,
        args: String,
        onComplete: @escaping (String) -> Void
    ) {
        switch command.id {
        case AppConstants.Launcher.Command.shell:
            guard !args.isEmpty else {
                onComplete("Usage: /shell <command>")
                return
            }
            commandFeedback = "Running..."
            ShellCommand.run(args) { [weak self] message in
                self?.commandFeedback = message
                onComplete(message)
            }

        case AppConstants.Launcher.Command.calc:
            guard !args.isEmpty else {
                onComplete("Usage: /calc <expression>")
                return
            }
            let result = CalcCommand.evaluate(args)
            switch result {
            case .value(let value):
                commandFeedback = "Result: \(value)"
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(value, forType: .string)
                onComplete("Result: \(value)")
            case .error(let message):
                onComplete(message)
            }

        case AppConstants.Launcher.Command.kill:
            let searchTerm = args.trimmingCharacters(in: .whitespacesAndNewlines)
            let matched = KillCommand.suggestions(searchTerm: searchTerm)

            if matched.isEmpty {
                if searchTerm.hasPrefix(":") || searchTerm.lowercased().hasPrefix("port ") {
                    commandFeedback = "No process listening on this port"
                } else {
                    commandFeedback = "No matching apps. /kill to list all. Use :3000 to search by port."
                }
                onComplete(commandFeedback)
            } else if searchTerm.isEmpty {
                let appList = matched.map { candidate in
                    "\(candidate.number). \(candidate.displayName) (PID: \(candidate.pid))"
                }
                commandFeedback = "Running apps:\n" + appList.joined(separator: "\n") + "\n\n/kill <name or number>"
                onComplete(commandFeedback)
            } else if matched.count > 1 {
                let list = matched.map { candidate in "\(candidate.number). \(candidate.displayName)" }
                commandFeedback = "Multiple matches:\n" + list.joined(separator: "\n") + "\n\nBe more specific."
                onComplete(commandFeedback)
            } else {
                let candidate = matched[0]
                KillCommand.kill(pid: candidate.pid, name: candidate.displayName) { [weak self] message in
                    self?.commandFeedback = message
                    onComplete(message)
                }
            }

        case AppConstants.Launcher.Command.sys:
            commandFeedback = ""
            onComplete("")

        default:
            onComplete("Unsupported command")
        }
    }

    func runKillCommand(num: Int, onComplete: @escaping (String) -> Void) {
        let candidates = KillCommand.suggestions(searchTerm: "")
        guard num > 0 && num <= candidates.count else {
            onComplete("Invalid app number")
            return
        }
        let candidate = candidates[num - 1]
        KillCommand.kill(pid: candidate.pid, name: candidate.displayName) { [weak self] message in
            self?.commandFeedback = message
            onComplete(message)
        }
    }

    func openResult(
        _ result: LauncherResult,
        showBanner: @escaping (String, BannerDisplayStyle, Double) -> Void,
        hideLauncher: @escaping () -> Void
    ) {
        switch result.kind {
        case .app:
            if openTarget(result.path, showBanner: showBanner) {
                if let error = bridge.recordUsage(candidateID: result.id, action: "open_app") {
                    showBanner(error.userFacingMessage, .info, 1.4)
                }
                hideLauncher()
            }
        case .file:
            if openTarget(result.path, showBanner: showBanner) {
                if let error = bridge.recordUsage(candidateID: result.id, action: "open_file") {
                    showBanner(error.userFacingMessage, .info, 1.4)
                }
                hideLauncher()
            }
        case .folder:
            if openTarget(result.path, showBanner: showBanner) {
                if !result.id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix),
                    let error = bridge.recordUsage(candidateID: result.id, action: "open_folder")
                {
                    showBanner(error.userFacingMessage, .info, 1.4)
                }
                hideLauncher()
            }
        case .clipboard:
            guard let content = result.clipboardContent, !content.isEmpty else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(content, forType: .string)
            showBanner(
                AppConstants.Launcher.Clipboard.copiedBanner,
                .success,
                AppConstants.Launcher.Clipboard.copiedBannerDuration
            )
        }
    }

    @discardableResult
    func openTarget(
        _ target: String,
        showBanner: @escaping (String, BannerDisplayStyle, Double) -> Void
    ) -> Bool {
        if target.contains(":") && !target.hasPrefix("/") {
            if let url = URL(string: target) {
                if NSWorkspace.shared.open(url) {
                    return true
                }
                showBanner("Could not open this item right now", .error, 1.2)
                return false
            }
            showBanner("Invalid target URL", .error, 1.2)
            return false
        }

        if NSWorkspace.shared.open(URL(fileURLWithPath: target)) {
            return true
        }

        showBanner("Could not open this path", .error, 1.2)
        return false
    }

    func revealInFinder(
        _ result: LauncherResult,
        showBanner: @escaping (String, BannerDisplayStyle, Double) -> Void
    ) {
        switch result.kind {
        case .app, .file, .folder:
            if result.path.contains(":") && !result.path.hasPrefix("/") {
                if let url = URL(string: result.path) {
                    NSWorkspace.shared.open(url)
                } else {
                    showBanner(
                        AppConstants.Launcher.Finder.cannotRevealBanner,
                        .info,
                        AppConstants.Launcher.Clipboard.infoBannerDuration
                    )
                }
            } else {
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: result.path)])
            }
        case .clipboard:
            showBanner(
                AppConstants.Launcher.Clipboard.nonFileBanner,
                .info,
                AppConstants.Launcher.Clipboard.infoBannerDuration
            )
        }
    }

    func copyToPasteboard(
        _ result: LauncherResult,
        showBanner: @escaping (String, BannerDisplayStyle, Double) -> Void
    ) -> Bool {
        guard result.kind == .file || result.kind == .folder else { return false }

        let targetURL = URL(fileURLWithPath: result.path)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let didWrite = pasteboard.writeObjects([targetURL as NSURL, result.path as NSString])

        if didWrite {
            showBanner("Copied \(result.kind.rawValue) to pasteboard", .success, 1.0)
        } else {
            showBanner("Copy failed", .error, 1.0)
        }

        return didWrite
    }
}

enum BannerDisplayStyle {
    case success
    case error
    case info

    var background: Color {
        switch self {
        case .success:
            return .green.opacity(0.42)
        case .error:
            return .red.opacity(0.45)
        case .info:
            return .blue.opacity(0.40)
        }
    }
}
