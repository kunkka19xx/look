import Foundation

struct ShellCommand {
    static func run(_ command: String, completion: @escaping (String) -> Void) {
        Task.detached(priority: .userInitiated) {
            let process = Process()
            let outputPipe = Pipe()
            process.executableURL = URL(fileURLWithPath: "/bin/zsh")
            process.arguments = ["-lc", command]
            process.standardOutput = outputPipe
            process.standardError = outputPipe

            let message: String
            do {
                try process.run()
                process.waitUntilExit()
                let data = outputPipe.fileHandleForReading.readDataToEndOfFile()
                let raw = String(data: data, encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
                if raw.isEmpty {
                    message = process.terminationStatus == 0 ? "Done" : "Error: command failed"
                } else {
                    let prefix = process.terminationStatus == 0 ? "" : "Error: "
                    message = String((prefix + raw).prefix(180))
                }
            } catch {
                message = "Error: failed to execute command"
            }

            await MainActor.run {
                completion(message)
            }
        }
    }

    static func hasSudoWarning(_ input: String) -> Bool {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }
        let pattern = "(^|\\s)sudo(\\s|$)"
        guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else {
            return false
        }
        let range = NSRange(trimmed.startIndex..<trimmed.endIndex, in: trimmed)
        return regex.firstMatch(in: trimmed, range: range) != nil
    }
}
