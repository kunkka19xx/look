import AppKit
import CoreServices
import SwiftUI
import UniformTypeIdentifiers

/// Trash-deletion of file/folder results, mirroring the Kill command's
/// confirm-then-act UX. Filtering and wording live in `DeleteTargetLogic`
/// (pure, unit-tested); this file owns the AppKit side: icons, the actual
/// `NSWorkspace.recycle` call, and the SwiftUI confirmation bar.
struct DeleteCommand {
    struct Target: Identifiable {
        let id: String
        let displayName: String
        let path: String
        let kind: LauncherResultKind
        let icon: NSImage?
    }

    struct Outcome {
        let trashedIDs: [String]
        let failures: [(id: String, name: String, reason: String)]

        var trashedCount: Int { trashedIDs.count }
        var firstFailure: (name: String, reason: String)? {
            failures.first.map { ($0.name, $0.reason) }
        }
    }

    /// Moves each target to the macOS Trash via `NSWorkspace.recycle`, reporting
    /// per-item success/failure so a partial failure is attributable. Recycle's
    /// completion fires on a background thread, so accumulation is lock-guarded;
    /// the final `Outcome` is delivered on the main queue.
    static func trash(_ targets: [Target], completion: @escaping (Outcome) -> Void) {
        guard !targets.isEmpty else {
            completion(Outcome(trashedIDs: [], failures: []))
            return
        }

        let lock = NSLock()
        var trashedIDs: [String] = []
        var failures: [(id: String, name: String, reason: String)] = []
        let group = DispatchGroup()

        for target in targets {
            group.enter()
            NSWorkspace.shared.recycle([URL(fileURLWithPath: target.path)]) { _, error in
                lock.lock()
                if let error {
                    failures.append((target.id, target.displayName, error.localizedDescription))
                } else {
                    trashedIDs.append(target.id)
                }
                lock.unlock()
                group.leave()
            }
        }

        group.notify(queue: .main) {
            completion(Outcome(trashedIDs: trashedIDs, failures: failures))
        }
    }
}

/// Empties the macOS Trash via Finder. `~/.Trash` is TCC-protected, so Look
/// can't enumerate/remove it directly without Full Disk Access — but Finder
/// already has the rights, so we drive it through AppleScript (which only needs
/// a one-time Automation permission). Irreversible, hence the confirm banner.
struct EmptyTrashCommand {
    /// Whether Look already has permission to automate Finder — checked WITHOUT
    /// prompting, so merely previewing the Trash doesn't pop a TCC dialog.
    @MainActor
    static func isAutomationAllowed() -> Bool {
        let target = NSAppleEventDescriptor(bundleIdentifier: "com.apple.finder")
        guard let desc = target.aeDesc else { return false }
        return AEDeterminePermissionToAutomateTarget(desc, typeWildCard, typeWildCard, false) == noErr
    }

    /// Number of items in the Trash, via Finder. Returns nil if Finder
    /// automation is unavailable/denied. Pass `promptIfNeeded: false` from
    /// passive contexts (preview) so it never triggers a permission prompt.
    @MainActor
    static func itemCount(promptIfNeeded: Bool = true) -> Int? {
        if !promptIfNeeded && !isAutomationAllowed() { return nil }
        let (result, error) = runFinder("return count of (items of the trash)")
        if error != nil { return nil }
        return result.flatMap { Int(exactly: $0.int32Value) }
    }

    /// Empties the Trash via Finder, off the main thread (it can take seconds on
    /// a large Trash). Delivers an error message on failure, nil on success, on
    /// the main queue. Suppresses Finder's own "are you sure" (we show our own
    /// confirm) and restores the user's preference afterward — the restore is
    /// isolated so it can't turn a successful empty into a reported failure.
    static func empty(completion: @escaping (String?) -> Void) {
        let body = """
        set prevWarn to warns before emptying of the trash
        set warns before emptying of the trash to false
        set emptyErr to missing value
        try
            empty the trash
        on error errMsg
            set emptyErr to errMsg
        end try
        try
            set warns before emptying of the trash to prevWarn
        end try
        if emptyErr is not missing value then error emptyErr
        """
        DispatchQueue.global(qos: .userInitiated).async {
            let error = runFinder(body).1
            DispatchQueue.main.async { completion(error) }
        }
    }

    nonisolated private static func runFinder(_ body: String) -> (NSAppleEventDescriptor?, String?) {
        let source = "tell application \"Finder\"\n\(body)\nend tell"
        guard let script = NSAppleScript(source: source) else {
            return (nil, "Could not build Finder script")
        }
        var errorInfo: NSDictionary?
        let result = script.executeAndReturnError(&errorInfo)
        if let errorInfo {
            let code = (errorInfo[NSAppleScript.errorNumber] as? Int) ?? 0
            // -1743 = user has not granted (or has denied) Automation permission.
            if code == -1743 {
                return (nil, "Allow Look to control Finder in System Settings ▸ Privacy ▸ Automation")
            }
            let msg = (errorInfo[NSAppleScript.errorMessage] as? String) ?? "Finder automation failed"
            return (nil, msg)
        }
        return (result, nil)
    }
}

/// Shared chrome for the destructive confirm banners (delete-to-Trash and
/// empty-Trash). Opaque backing so it reads over the results list; danger-tinted
/// border + shadow mark it as a destructive prompt.
struct ConfirmActionBar: View {
    let icon: NSImage
    let title: String
    let detail: String
    let themeStore: ThemeStore
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(nsImage: icon)
                .resizable()
                .frame(width: 24, height: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                Text(detail)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .lineLimit(1)
                    .truncationMode(.middle)
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
        .background(.ultraThickMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(themeStore.dangerColor().opacity(0.85), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.35), radius: 12, y: 4)
    }
}

struct EmptyTrashConfirmationBar: View {
    let itemCount: Int
    let themeStore: ThemeStore
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        ConfirmActionBar(
            icon: NSImage(named: NSImage.trashFullName) ?? NSWorkspace.shared.icon(for: .folder),
            title: "Empty Trash?",
            detail: DeleteTargetLogic.emptyTrashDetail(itemCount: itemCount),
            themeStore: themeStore,
            onConfirm: onConfirm,
            onCancel: onCancel
        )
    }
}
