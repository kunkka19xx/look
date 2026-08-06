import Foundation

/// Minimal markdown segmentation for chat answers: splits fenced code blocks
/// (``` ... ```) from prose, so the UI can render code in a monospaced card and
/// pass prose through inline-markdown styling. Deliberately not a full markdown
/// engine (no dependency); works mid-stream (an unclosed fence renders as code).
nonisolated enum ChatMarkdown {
    enum Segment: Equatable {
        case text(String)
        case code(String)
    }

    static func segments(from raw: String) -> [Segment] {
        var segments: [Segment] = []
        var buffer: [String] = []
        var inCode = false

        func flush() {
            let joined = buffer.joined(separator: "\n")
            buffer.removeAll()
            guard !joined.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
            segments.append(inCode ? .code(joined) : .text(joined))
        }

        for line in raw.components(separatedBy: "\n") {
            if line.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                flush()
                inCode.toggle()
            } else {
                buffer.append(line)
            }
        }
        flush()
        return segments
    }
}
