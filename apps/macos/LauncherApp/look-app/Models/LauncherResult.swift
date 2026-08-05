import Foundation

enum LauncherResultKind: String, Codable {
    case app
    case file
    case folder
    case clipboard
    /// A running process row from the `ps"` finder. Carries `processPID` /
    /// `processPorts`; detail (cmdline, memory, …) loads per-selection.
    case process
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
    /// What re-copying a clipboard row actually pastes, when it differs from
    /// `clipboardContent` (a labeled entry like `2+2 = 4` pastes `4`).
    var clipboardPayload: String? = nil
    /// Set only for `.process` rows: the process id and its listening TCP ports.
    var processPID: Int32? = nil
    var processPorts: [Int]? = nil
    /// Set only for the synthetic calculator row: the expression it was parsed
    /// from and the raw value pressing Enter copies. `title` carries the
    /// grouped display value.
    var calcExpression: String? = nil
    var calcRawValue: String? = nil
}
