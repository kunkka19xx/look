import Foundation

enum LauncherResultKind: String, Codable {
    case app
    case file
    case folder
    case clipboard
    /// A running process row from the `ps"` finder. Carries `processPID` /
    /// `processPorts`; detail (cmdline, memory, …) loads per-selection.
    case process
    /// A row from a user-declared block that has no filesystem target: Enter
    /// performs its steps rather than opening anything.
    case action

    /// Filesystem targets - the only kinds `Cmd+D`/`Cmd+P`/pick-to-pasteboard
    /// operate on.
    var isFileOrFolder: Bool {
        self == .file || self == .folder
    }
}

struct LauncherResult: Identifiable {
    let id: String
    let kind: LauncherResultKind
    let title: String
    let subtitle: String?
    let path: String
    /// `var`, not `let`: most rows are built with their final rank, but the
    /// call rows order themselves after the fact. Its POSITION is load-bearing
    /// - the memberwise initializer is called positionally all over the app.
    var score: Int
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
    /// Set on the synthetic rows that open a URL (a meeting to join, a way to
    /// reach a person): what the preview shows without re-parsing the subtitle
    /// it was written into. The URL itself rides in the result id.
    var linkKindLabel: String? = nil
    var linkDetail: String? = nil
}

extension LauncherResult {
    /// A row a user-declared block produced (`specs/user-sources.md`). One
    /// definition: the prefix was spelled out in three views, which is how a
    /// namespace check drifts.
    var isSourceRow: Bool {
        id.hasPrefix(AppConstants.Launcher.SourceBlock.idPrefix)
    }
}
