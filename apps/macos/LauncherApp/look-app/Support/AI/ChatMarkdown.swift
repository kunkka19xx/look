import Foundation

/// Minimal markdown segmentation for chat answers: splits fenced code blocks
/// (``` ... ```) from prose, so the UI can render code in a monospaced card and
/// pass prose through inline-markdown styling. Deliberately not a full markdown
/// engine (no dependency); works mid-stream (an unclosed fence renders as code).
nonisolated enum ChatMarkdown {
    enum Segment: Equatable {
        case text(String)
        case code(String, language: String?)
    }

    static func segments(from raw: String) -> [Segment] {
        var segments: [Segment] = []
        var buffer: [String] = []
        var inCode = false
        var codeLanguage: String?

        func flush() {
            let joined = buffer.joined(separator: "\n")
            buffer.removeAll()
            guard !joined.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
            segments.append(inCode ? .code(joined, language: codeLanguage) : .text(joined))
        }

        for line in raw.components(separatedBy: "\n") {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("```") {
                flush()
                if !inCode {
                    // Opening fence may carry a language tag: ```swift
                    let tag = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
                    codeLanguage = tag.isEmpty
                        ? nil
                        : tag.split(separator: " ").first.map { String($0).lowercased() }
                } else {
                    codeLanguage = nil
                }
                inCode.toggle()
            } else {
                buffer.append(line)
            }
        }
        flush()
        return segments
    }
}
