import Foundation

enum LauncherResultKind: String, Codable {
    case app
    case file
    case folder
    case clipboard
}

struct LauncherResult: Identifiable {
    let id: String
    let kind: LauncherResultKind
    let title: String
    let subtitle: String?
    let path: String
    let score: Int
    var clipboardContent: String? = nil
    var clipboardCapturedAt: Date? = nil
    var clipboardCharacterCount: Int? = nil
    var clipboardLineCount: Int? = nil
}
