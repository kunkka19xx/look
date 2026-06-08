import AppKit
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

struct DeleteConfirmationBar: View {
    let targets: [DeleteCommand.Target]
    let themeStore: ThemeStore
    let onConfirm: () -> Void
    let onCancel: () -> Void

    private var title: String {
        DeleteTargetLogic.confirmTitle(displayNames: targets.map(\.displayName))
    }

    private var detail: String {
        DeleteTargetLogic.confirmDetail(
            fileCount: targets.filter { $0.kind == .file }.count,
            folderCount: targets.filter { $0.kind == .folder }.count,
            singlePath: targets.count == 1 ? targets[0].path : nil
        )
    }

    private var icon: NSImage {
        targets.first?.icon ?? NSWorkspace.shared.icon(for: .folder)
    }

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
        // This bar overlays the results list (unlike the kill bar, which floats
        // over empty command-mode space), so it needs an opaque backing or the
        // list bleeds through and the text becomes unreadable. A thick material
        // obscures content behind; the danger-tinted border + shadow mark it as
        // a destructive prompt and lift it off the list.
        .background(.ultraThickMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .background(themeStore.controlFillColor(), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(themeStore.dangerColor().opacity(0.85), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.35), radius: 12, y: 4)
    }
}
