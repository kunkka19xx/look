import Foundation

enum ClipboardContentType: String, Codable {
    case text
    case image
    case fileList = "file_list"

    var icon: String {
        switch self {
        case .text: return "doc.on.clipboard"
        case .image: return "photo"
        case .fileList: return "folder"
        }
    }

    var label: String {
        switch self {
        case .text: return "Text"
        case .image: return "Image"
        case .fileList: return "Files"
        }
    }
}

struct ClipboardEntry: Identifiable, Decodable {
    let id: String
    let contentType: ClipboardContentType
    let content: String
    let preview: String?
    let sourceApp: String?
    let createdAtUnixS: Int64
    let lastUsedAtUnixS: Int64?
    let useCount: Int
    let pinned: Bool

    enum CodingKeys: String, CodingKey {
        case id
        case contentType = "content_type"
        case content
        case preview
        case sourceApp = "source_app"
        case createdAtUnixS = "created_at_unix_s"
        case lastUsedAtUnixS = "last_used_at_unix_s"
        case useCount = "use_count"
        case pinned
    }

    var displayText: String {
        preview ?? content
    }

    var relativeTime: String {
        let now = Int64(Date().timeIntervalSince1970)
        let diff = now - createdAtUnixS
        if diff < 60 { return "just now" }
        if diff < 3600 { return "\(diff / 60)m ago" }
        if diff < 86400 { return "\(diff / 3600)h ago" }
        return "\(diff / 86400)d ago"
    }
}
