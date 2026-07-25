import Combine
import SwiftUI

/// The launchpad's L slot (2x2). Priority-driven, with a crossfade when the
/// active source changes:
///
/// - Pomo when a Pomodoro session is running (highest priority).
/// - Todo when no session runs but tasks remain today.
/// - Clock as the fallback when neither applies.
///
/// This is the one launchpad tile wired to live state in the layout pass, since
/// the Todo and Pomo stores already exist.
struct LaunchpadLSlotView: View {
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    private enum Slot: Equatable { case todo, pomo, clock }

    /// Pomo wins whenever a session is running; else Todo when tasks remain;
    /// else the Clock fallback.
    private var current: Slot {
        if PomoSharedState.shared.activeIndex != nil { return .pomo }
        if todoRemaining > 0 { return .todo }
        return .clock
    }

    private var todoRemaining: Int {
        TodoSharedState.shared.today?.openCount ?? 0
    }

    var body: some View {
        ZStack {
            switch current {
            case .todo:
                LaunchpadTodoTile(themeStore: themeStore)
            case .pomo:
                LaunchpadPomoTile(themeStore: themeStore)
            case .clock:
                LaunchpadClockTile(themeStore: themeStore)
            }
        }
        .id(current)
        .transition(.opacity)
        .animation(.easeInOut(duration: Const.rotateFadeSeconds), value: current)
    }
}

// MARK: - Todo tile

/// Done/total today plus a rotating next-task name (cycles per the spec).
private struct LaunchpadTodoTile: View {
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    @State private var taskIndex = 0

    private let taskTimer = Timer.publish(
        every: AppConstants.Launcher.Launchpad.todoTaskRotateSeconds,
        on: .main,
        in: .common
    ).autoconnect()

    private var stat: TodoStat { TodoSharedState.shared.todayStat }
    private var openTasks: [String] {
        TodoSharedState.shared.today?.tasks.filter { !$0.done }.map(\.name) ?? []
    }

    var body: some View {
        LaunchpadSlotCard(themeStore: themeStore, iconName: "checklist", showsClock: true) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text("\(stat.done)/\(stat.total)")
                        .font(themeStore.uiFont(size: 26, weight: .bold))
                        .foregroundColor(themeStore.fontColor())
                    Text("done today")
                        .font(themeStore.uiFont(size: Const.titleFontSize))
                        .foregroundColor(themeStore.secondaryTextColor())
                }
                HStack(spacing: 6) {
                    Circle()
                        .fill(themeStore.accentColor())
                        .frame(width: 5, height: 5)
                    Text(nextTaskLabel)
                        .font(themeStore.uiFont(size: Const.captionFontSize + 1))
                        .foregroundColor(themeStore.secondaryTextColor())
                        .lineLimit(1)
                        .id(taskIndex)
                        .transition(.opacity)
                }
                .animation(.easeInOut(duration: 0.35), value: taskIndex)
            }
        }
        .onReceive(taskTimer) { _ in
            guard openTasks.count > 1 else { return }
            taskIndex = (taskIndex + 1) % openTasks.count
        }
    }

    private var nextTaskLabel: String {
        let tasks = openTasks
        guard !tasks.isEmpty else { return "All clear" }
        return tasks[taskIndex % tasks.count]
    }
}

// MARK: - Pomo tile

/// Live countdown, phase + session, and a progress bar for the running session.
private struct LaunchpadPomoTile: View {
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    private var state: PomoState { PomoSharedState.shared }

    var body: some View {
        LaunchpadSlotCard(themeStore: themeStore, iconName: "timer", showsClock: true) {
            VStack(alignment: .leading, spacing: 8) {
                Text(PomoCommand.formattedRemaining(state.secondsLeft))
                    .font(themeStore.uiFont(size: 30, weight: .bold))
                    .foregroundColor(themeStore.fontColor())
                    .monospacedDigit()
                Text(phaseLabel)
                    .font(themeStore.uiFont(size: Const.captionFontSize + 1))
                    .foregroundColor(themeStore.secondaryTextColor())
                ProgressView(value: min(max(state.progress, 0), 1))
                    .tint(themeStore.accentColor())
                    .frame(height: 3)
            }
        }
    }

    private var phaseLabel: String {
        let phase = state.currentSession?.type == .focus ? "Focus" : "Break"
        guard let index = state.activeIndex else { return phase }
        return "\(phase) - session \(index + 1)/\(state.sessions.count)"
    }
}

// MARK: - Clock tile

/// Live time + date fallback, with today's lunar date in the top-right corner
/// (read from the shared `core/lunar` crate). Non-sensitive; safe during
/// screen-share.
private struct LaunchpadClockTile: View {
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    @State private var now = Date()
    @State private var lunar: LunarDate?

    private let tickTimer = Timer.publish(
        every: AppConstants.Launcher.Launchpad.clockTickSeconds,
        on: .main,
        in: .common
    ).autoconnect()

    var body: some View {
        LaunchpadSlotCard(
            themeStore: themeStore,
            iconName: "clock",
            headerTrailing: lunar.map { AnyView(LunarBadge(lunar: $0, themeStore: themeStore)) }
        ) {
            VStack(alignment: .leading, spacing: 6) {
                Text(now, format: .dateTime.hour().minute())
                    .font(themeStore.uiFont(size: 30, weight: .bold))
                    .foregroundColor(themeStore.fontColor())
                    .monospacedDigit()
                Text(now, format: .dateTime.weekday().month().day())
                    .font(themeStore.uiFont(size: Const.titleFontSize))
                    .foregroundColor(themeStore.secondaryTextColor())
            }
        }
        .onAppear { refreshLunar(for: now) }
        .onReceive(tickTimer) { tick in
            // Recompute only when the calendar day rolls over (the lunar date is
            // stable within a day), or if the first read hasn't landed yet.
            if lunar == nil || !Calendar.current.isDate(tick, inSameDayAs: now) {
                refreshLunar(for: tick)
            }
            now = tick
        }
    }

    private func refreshLunar(for date: Date) {
        lunar = LunarToday.resolve(for: date)
    }
}

/// Resolves a Gregorian date to its lunar date via the shared core, at the
/// viewer's local UTC offset. Shared by the clock tile and the header clock so
/// the conversion lives in one place.
private enum LunarToday {
    static func resolve(for date: Date) -> LunarDate? {
        let parts = Calendar.current.dateComponents([.year, .month, .day], from: date)
        guard let year = parts.year, let month = parts.month, let day = parts.day else { return nil }
        let tzHours = Double(TimeZone.current.secondsFromGMT(for: date)) / 3600.0
        return EngineBridge.shared.lunarDate(year: year, month: month, day: day, tzHours: tzHours)
    }

    /// The compact one-line label used in the header clock, e.g. "12/6 Lunar".
    static func inlineLabel(_ lunar: LunarDate) -> String {
        let name = lunar.leap
            ? AppConstants.Launcher.Launchpad.lunarLeapLabel
            : AppConstants.Launcher.Launchpad.lunarLabel
        return "\(lunar.day)/\(lunar.month) \(name)"
    }
}

/// Today's lunar day/month plus a "Lunar" (or "Lunar leap") caption, shown in the
/// clock tile's header corner.
private struct LunarBadge: View {
    let lunar: LunarDate
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    var body: some View {
        VStack(alignment: .trailing, spacing: 0) {
            Text("\(lunar.day)/\(lunar.month)")
                .font(themeStore.uiFont(size: Const.captionFontSize + 5, weight: .semibold))
                .foregroundColor(themeStore.secondaryTextColor())
                .monospacedDigit()
            Text(lunar.leap ? Const.lunarLeapLabel : Const.lunarLabel)
                .font(themeStore.uiFont(size: Const.captionFontSize - 1))
                .foregroundColor(themeStore.mutedTextColor())
        }
    }
}

// MARK: - Header clock

/// Time + Gregorian date (full strength) with the lunar date dimmed below, kept
/// visible while the Todo or Pomo slot occupies the tile.
private struct LaunchpadHeaderClock: View {
    var themeStore: ThemeStore

    private typealias Const = AppConstants.Launcher.Launchpad

    @State private var now = Date()
    @State private var lunar: LunarDate?

    private let tickTimer = Timer.publish(
        every: AppConstants.Launcher.Launchpad.clockTickSeconds,
        on: .main,
        in: .common
    ).autoconnect()

    var body: some View {
        VStack(alignment: .trailing, spacing: Const.headerClockLineSpacing) {
            Text(now, format: .dateTime.hour().minute())
                .font(themeStore.uiFont(size: Const.headerClockTimeFontSize, weight: .semibold))
                .foregroundColor(themeStore.secondaryTextColor())
                .monospacedDigit()
            Text(now, format: .dateTime.weekday().month().day())
                .font(themeStore.uiFont(size: Const.headerClockDateFontSize))
                .foregroundColor(themeStore.secondaryTextColor())
            if let lunar {
                Text(LunarToday.inlineLabel(lunar))
                    .font(themeStore.uiFont(size: Const.headerClockDateFontSize))
                    .foregroundColor(themeStore.mutedTextColor())
                    .monospacedDigit()
            }
        }
        .onAppear { lunar = LunarToday.resolve(for: now) }
        .onReceive(tickTimer) { tick in
            if lunar == nil || !Calendar.current.isDate(tick, inSameDayAs: now) {
                lunar = LunarToday.resolve(for: tick)
            }
            now = tick
        }
    }
}

// MARK: - Shared L-slot card chrome

/// The common 2x2 card frame: an icon badge in the top-left, an optional header
/// readout in the top-right, and the slot's content pinned to the bottom. When
/// `showsClock` is set, a compact time + date sits in the header so it stays
/// visible even while the Todo or Pomo slot occupies the tile.
private struct LaunchpadSlotCard<Content: View>: View {
    var themeStore: ThemeStore
    let iconName: String
    var showsClock: Bool = false
    /// Optional readout pinned to the header's top-right (e.g. the clock tile's
    /// lunar date). Stays inside this card, beside the icon.
    var headerTrailing: AnyView? = nil
    @ViewBuilder var content: () -> Content

    private typealias Const = AppConstants.Launcher.Launchpad

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                ZStack {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .fill(themeStore.selectionFillColor())
                    Image(systemName: iconName)
                        .font(.system(size: 20, weight: .medium))
                        .foregroundColor(themeStore.accentColor())
                }
                .frame(width: 40, height: 40)
                Spacer(minLength: 0)
                if showsClock {
                    LaunchpadHeaderClock(themeStore: themeStore)
                }
                if let headerTrailing {
                    headerTrailing
                }
            }
            Spacer(minLength: 0)
            content()
        }
        .padding(14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(frostedTile(themeStore: themeStore))
        .overlay(
            RoundedRectangle(cornerRadius: Const.cornerRadius, style: .continuous)
                .strokeBorder(themeStore.dividerColor().opacity(0.4), lineWidth: 1)
        )
    }
}
