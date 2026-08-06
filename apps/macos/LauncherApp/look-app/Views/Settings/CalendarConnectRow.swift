import SwiftUI

/// Settings row to grant EventKit access and show current status. Self-contained
/// (owns its state) so it drops into the AI settings block without touching the
/// main settings struct.
struct CalendarConnectRow: View {
    let themeStore: ThemeStore
    @State private var status: String = ""

    var body: some View {
        HStack(spacing: 10) {
            Text("Calendar & Reminders")
                .frame(width: AppConstants.ThemeUI.labelWidth, alignment: .leading)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                .foregroundStyle(themeStore.secondaryTextColor())

            Button("Connect") {
                Task {
                    await EventKitService.shared.requestAccess()
                    refresh()
                }
            }
            .help("Grant Look access to create events and reminders in your system calendar")

            Text(status)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())

            Spacer(minLength: 0)
        }
        .onAppear(perform: refresh)
    }

    private func refresh() {
        status = EventKitService.shared.calendarAccess.message
    }
}
