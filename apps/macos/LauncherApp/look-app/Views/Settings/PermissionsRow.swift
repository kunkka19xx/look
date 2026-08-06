import AppKit
import SwiftUI

/// One Settings row holding a chip per capability that needs OS access. Scales by
/// adding chips, not rows: a future connector (Contacts, Photos, ...) is one more
/// `PermissionChip` here, mirroring the action-tool registry.
struct PermissionsRow: View {
    let themeStore: ThemeStore
    @State private var calendarStatus: CalendarAccess = .notDetermined

    var body: some View {
        HStack(spacing: 10) {
            Text("Permissions")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            PermissionChip(
                label: "Calendar",
                granted: calendarStatus.canWrite,
                themeStore: themeStore
            ) {
                if calendarStatus.canWrite {
                    // Apps cannot revoke their own privacy grant; only the user
                    // can, in System Settings. Open the pane for them.
                    openSystemPrivacyCalendars()
                } else {
                    Task {
                        await EventKitService.shared.requestAccess()
                        refresh()
                    }
                }
            }

            // Future capabilities add a chip here, not a new row.

            Spacer(minLength: 0)
        }
        .onAppear(perform: refresh)
    }

    private func refresh() {
        calendarStatus = EventKitService.shared.calendarAccess
    }

    private func openSystemPrivacyCalendars() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars") {
            NSWorkspace.shared.open(url)
        }
    }
}

/// A tappable capsule showing a capability's name and grant state. Green dot when
/// connected; shows "Connect" and requests access when not.
struct PermissionChip: View {
    let label: String
    let granted: Bool
    let themeStore: ThemeStore
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Circle()
                    .fill(granted ? Color.green.opacity(0.85) : themeStore.mutedTextColor())
                    .frame(width: 7, height: 7)
                Text(label)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .medium))
                    .foregroundStyle(themeStore.fontColor())
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(themeStore.controlFillColor(), in: Capsule())
        }
        .buttonStyle(.plain)
        .help(granted
            ? "\(label) connected. Open System Settings to revoke access."
            : "Connect \(label)")
    }
}
