import Foundation

/// The symbol for a row whose action is "open this URL". Derived from the
/// scheme rather than stored on the row: the id already carries the URL, and a
/// second copy of "what kind of link is this" is a second thing to keep true.
nonisolated enum LinkRowAppearance {
    private enum Symbol {
        static let message = "message.fill"
        static let phone = "phone.fill"
        static let video = "video.fill"
    }

    static func symbol(forURL url: String) -> String {
        let lower = url.lowercased()
        if lower.hasPrefix("sms:") || lower.hasPrefix("imessage:") { return Symbol.message }
        if lower.hasPrefix("tel:") { return Symbol.phone }
        // FaceTime audio and video, and every conferencing link.
        return Symbol.video
    }
}
