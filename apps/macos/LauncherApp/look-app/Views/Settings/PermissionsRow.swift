import AppKit
import SwiftUI

/// One capability that needs macOS access, what Look does with it, and where it
/// lives in System Settings once it has been answered.
///
/// macOS has no "grant everything" API: each family prompts on its own, from
/// its own call. `Grant all` therefore walks these in order rather than opening
/// one dialog. Automation is absent on purpose - it cannot be requested without
/// actually sending an Apple event, so it stays a first-use prompt.
nonisolated struct PermissionItem: Identifiable {
    enum Capability: String {
        case calendar
        case reminders
    }

    let id: Capability
    let title: String
    /// What Look does with it. Lives in the tooltip rather than the row: the
    /// settings panel is a column of one-line controls, and a paragraph per
    /// permission pushed everything below it off the screen.
    let purpose: String
    /// The System Settings pane that owns it, for when only the user can change
    /// the answer.
    let settingsPane: String

    static let all: [PermissionItem] = [
        PermissionItem(
            id: .calendar,
            title: "Calendar",
            purpose: "Add, move, and join meetings",
            settingsPane: "Privacy_Calendars"),
        PermissionItem(
            id: .reminders,
            title: "Reminders",
            purpose: "Add, complete, and snooze reminders",
            settingsPane: "Privacy_Reminders"),
    ]
}

/// One Settings row holding a chip per capability that needs OS access. Scales
/// by adding chips, not rows: a future connector (Contacts, Photos, ...) is one
/// more entry in `PermissionItem.all`.
struct PermissionsRow: View {
    let themeStore: ThemeStore

    @State private var states: [PermissionItem.Capability: CalendarAccess] = [:]
    @State private var isGranting = false

    private func state(_ item: PermissionItem) -> CalendarAccess {
        states[item.id, default: .notDetermined]
    }

    /// Capabilities that have never been answered. Only these can still be
    /// prompted: macOS ignores a second request for one already decided.
    private var unanswered: [PermissionItem] {
        PermissionItem.all.filter { state($0) == .notDetermined }
    }

    var body: some View {
        HStack(spacing: 8) {
            Text("Permissions")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            ForEach(PermissionItem.all) { item in
                PermissionChip(
                    label: item.title,
                    granted: state(item) == .authorized,
                    themeStore: themeStore,
                    help: helpText(item)
                ) {
                    act(on: item)
                }
            }

            // Offered only while something can still be asked, so it is never a
            // button that silently does nothing.
            if !unanswered.isEmpty {
                Button(isGranting ? "Asking…" : "Grant all") { grantAll() }
                    .buttonStyle(.plain)
                    .disabled(isGranting)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                    .foregroundStyle(themeStore.accentColor())
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(themeStore.controlFillColor(), in: Capsule())
                    .help("Ask for each remaining permission in turn")
            }

            Spacer(minLength: 0)
        }
        .onAppear(perform: refresh)
        // A grant made in System Settings while Look is open should show up on
        // the way back, without a relaunch.
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
        ) { _ in refresh() }
    }

    /// The chip carries a dot and a name, so the tooltip has to say what this
    /// access is for and what clicking will do - which differs by state.
    private func helpText(_ item: PermissionItem) -> String {
        switch state(item) {
        case .authorized:
            return "\(item.purpose). Connected - open System Settings to revoke."
        case .writeOnly:
            return "\(item.purpose). Partial access: open System Settings to allow reading too."
        case .notDetermined:
            return "\(item.purpose). Click to connect."
        case .denied, .restricted:
            return "\(item.purpose). Denied - only System Settings can change it."
        }
    }

    private func act(on item: PermissionItem) {
        switch state(item) {
        case .notDetermined:
            Task {
                await request(item.id)
                refresh()
            }
        // Answered either way, so only the user can change it, and only there.
        case .authorized, .writeOnly, .denied, .restricted:
            openSettings(pane: item.settingsPane)
        }
    }

    /// Walks the unanswered capabilities in turn. Sequential on purpose: the
    /// prompts are modal one at a time, and firing them together would stack
    /// dialogs the user cannot read.
    private func grantAll() {
        isGranting = true
        Task {
            for item in unanswered {
                await request(item.id)
                refresh()
            }
            isGranting = false
        }
    }

    private func request(_ capability: PermissionItem.Capability) async {
        switch capability {
        case .calendar: await EventKitService.shared.requestCalendarAccess()
        case .reminders: await EventKitService.shared.requestReminderAccess()
        }
    }

    private func refresh() {
        states = [
            .calendar: EventKitService.shared.calendarAccess,
            .reminders: EventKitService.shared.reminderAccess,
        ]
    }

    private func openSettings(pane: String) {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?\(pane)")
        else { return }
        NSWorkspace.shared.open(url)
    }
}

/// A tappable capsule showing a capability's name and grant state. Green dot
/// when connected; clicking connects it, or opens System Settings once macOS
/// has an answer on file.
struct PermissionChip: View {
    let label: String
    let granted: Bool
    let themeStore: ThemeStore
    let help: String
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
        .help(help)
    }
}
