import SwiftUI

// Week / month done-total donuts, a current streak, a 30-day completion
// trend line, and a GitHub-style activity heatmap. Series come from
// TodoAnalytics (deterministic placeholders until storage exists).

struct TodoAnalyticsPage: View {
    let themeStore: ThemeStore

    private let trend = TodoAnalytics.monthTrend()
    private let heatDays = TodoAnalytics.heatmapDays()

    var body: some View {
        // Fill the height on large screens by letting the four sections
        // spread apart, while still scrolling (at a compact min gap) when
        // the panel is too short to fit them.
        GeometryReader { geo in
            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    statStrip
                    Spacer(minLength: 14)
                    trendSection
                    Spacer(minLength: 14)
                    heatmapSection
                    Spacer(minLength: 14)
                    insightsSection
                }
                .padding(.horizontal, 4)
                .padding(.vertical, 2)
                .frame(minHeight: geo.size.height, alignment: .top)
            }
        }
    }

    private var statStrip: some View {
        TodoStatStrip(
            themeStore: themeStore,
            week: TodoAnalytics.week,
            month: TodoAnalytics.month,
            streak: TodoAnalytics.streakDays
        )
    }

    private var trendSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionLabel("chart.bar", "Completion trend · 30 days")
            VStack(spacing: 4) {
                TodoLineChart(data: trend, themeStore: themeStore)
                    .frame(height: 92)
                HStack {
                    Text("Jun 5")
                    Spacer()
                    Text("Jun 20")
                    Spacer()
                    Text("Jul 4")
                }
                .font(.system(size: 9.5, design: .monospaced))
                .foregroundStyle(themeStore.mutedTextColor())
            }
            .padding(.horizontal, 12)
            .padding(.top, 10)
            .padding(.bottom, 6)
            .todoCard(themeStore)
        }
    }

    private var heatmapSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .center) {
                sectionLabel("calendar", "Activity · last year")
                Spacer()
                TodoHeatLegend(themeStore: themeStore)
            }
            TodoHeatmap(columns: heatDays, themeStore: themeStore)
                .padding(12)
                .todoCard(themeStore)
        }
    }

    private var insightsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionLabel("sparkles", "Insights · last 30 days (Tasks)")
            TodoInsightsStrip(themeStore: themeStore, trend: trend)
        }
    }

    private func sectionLabel(_ icon: String, _ text: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
                .font(.system(size: 11))
                .foregroundStyle(themeStore.mutedTextColor())
            Text(text.uppercased())
                .font(.system(size: 11, design: .monospaced))
                .tracking(0.6)
                .foregroundStyle(themeStore.secondaryTextColor())
        }
    }
}

struct TodoInsightsStrip: View {
    let themeStore: ThemeStore
    let trend: [Int]

    private var total: Int { trend.reduce(0, +) }
    private var avgPerDay: String {
        guard !trend.isEmpty else { return "0" }
        return String(format: "%.1f", Double(total) / Double(trend.count))
    }
    private var bestDay: Int { trend.max() ?? 0 }
    private var activeDays: Int { trend.filter { $0 > 0 }.count }

    var body: some View {
        HStack(spacing: 4) {
            tile(
                "Avg / day", avgPerDay,
                help: "Average tasks completed per day over the last 30 days")
            divider
            tile("Best day", "\(bestDay)", help: "Most tasks completed in a single day")
            divider
            tile(
                "Active days", "\(activeDays)/\(trend.count)",
                help: "Days with at least one task completed")
            divider
            tile("Done · 30d", "\(total)", help: "Total tasks completed in the last 30 days")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 14)
        .todoCard(themeStore, cornerRadius: 12)
    }

    private func tile(_ label: String, _ value: String, help: String) -> some View {
        VStack(spacing: 4) {
            Text(value)
                .font(themeStore.uiFont(size: 20, weight: .bold))
                .foregroundStyle(themeStore.fontColor())
            Text(label.uppercased())
                .font(.system(size: 10, design: .monospaced))
                .tracking(0.7)
                .foregroundStyle(themeStore.mutedTextColor())
        }
        .frame(maxWidth: .infinity)
        .help(help)
    }

    private var divider: some View { TodoVDivider(themeStore: themeStore) }
}

struct TodoStatStrip: View {
    let themeStore: ThemeStore
    let week: TodoStat
    let month: TodoStat
    let streak: Int

    var body: some View {
        HStack(alignment: .top, spacing: 4) {
            TodoMetricColumn(themeStore: themeStore, label: "This week", stat: week)
            divider
            TodoMetricColumn(themeStore: themeStore, label: "This month", stat: month)
            divider
            TodoStreakColumn(themeStore: themeStore, days: streak)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 14)
        .todoCard(themeStore, cornerRadius: 12)
    }

    private var divider: some View { TodoVDivider(themeStore: themeStore) }
}

// Shared caption used for each column's title.
private func metricTitle(_ text: String, _ themeStore: ThemeStore) -> some View {
    Text(text.uppercased())
        .font(.system(size: 12, design: .monospaced))
        .tracking(0.7)
        .foregroundStyle(themeStore.mutedTextColor())
}

struct TodoMetricColumn: View {
    let themeStore: ThemeStore
    let label: String
    let stat: TodoStat

    private var percent: Int { Int((stat.fraction * 100).rounded()) }

    var body: some View {
        VStack(spacing: 10) {
            HStack(spacing: 5) {
                metricTitle(label, themeStore)
                Text("\(percent)%")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(themeStore.accentColor())
            }
            ZStack {
                TodoDonut(fraction: stat.fraction, themeStore: themeStore, size: 62)
                VStack(spacing: 2) {
                    Text("\(stat.done)")
                        .font(themeStore.uiFont(size: 16, weight: .bold))
                        .foregroundStyle(themeStore.fontColor())
                    Rectangle()
                        .fill(themeStore.dividerColor())
                        .frame(width: 16, height: 1)
                    Text("\(stat.total)")
                        .font(themeStore.uiFont(size: 12))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
            }
        }
        .frame(maxWidth: .infinity)
    }
}

struct TodoStreakColumn: View {
    let themeStore: ThemeStore
    let days: Int

    var body: some View {
        VStack(spacing: 8) {
            metricTitle("Streak", themeStore)

            HStack(spacing: 12) {
                Image(systemName: "flame.fill")
                    .font(.system(size: 24))
                    .foregroundStyle(themeStore.accentColor())
                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .firstTextBaseline, spacing: 3) {
                        Text("\(days)")
                            .font(themeStore.uiFont(size: 18, weight: .bold))
                            .foregroundStyle(themeStore.fontColor())
                        Text("days")
                            .font(themeStore.uiFont(size: 12))
                            .foregroundStyle(themeStore.secondaryTextColor())
                    }
                    HStack(spacing: 4) {
                        let filled = min(days, 7)
                        ForEach(0..<7, id: \.self) { i in
                            let on = i >= 7 - filled
                            Circle()
                                .fill(on ? themeStore.accentColor() : Color.clear)
                                .overlay(
                                    Circle().stroke(
                                        on ? Color.clear : themeStore.dividerColor(), lineWidth: 1)
                                )
                                .frame(width: 6, height: 6)
                        }
                    }
                }
            }
            .frame(height: 62)
        }
        .frame(maxWidth: .infinity)
    }
}

/// Thin progress ring. The percentage is conveyed by the arc and the
/// exact counts sit beside it (see TodoMetricColumn), so the ring keeps
/// no redundant center label.
struct TodoDonut: View {
    let fraction: Double
    let themeStore: ThemeStore
    var size: CGFloat = 52
    var stroke: CGFloat = 4.5

    var body: some View {
        ZStack {
            Circle().stroke(themeStore.dividerColor(), lineWidth: stroke)
            Circle()
                .trim(from: 0, to: max(0, min(1, fraction)))
                .stroke(
                    themeStore.accentColor(), style: StrokeStyle(lineWidth: stroke, lineCap: .round)
                )
                .rotationEffect(.degrees(-90))
        }
        .frame(width: size, height: size)
    }
}

struct TodoLineChart: View {
    let data: [Int]
    let themeStore: ThemeStore

    var body: some View {
        Canvas { ctx, size in
            guard data.count > 1 else { return }
            let padL: CGFloat = 4
            let padR: CGFloat = 4
            let padT: CGFloat = 10
            let padB: CGFloat = 8
            let w = size.width - padL - padR
            let h = size.height - padT - padB
            let maxV = CGFloat(max(TodoCommand.taskLimit, data.max() ?? TodoCommand.taskLimit))
            let stepX = w / CGFloat(data.count - 1)

            let pts: [CGPoint] = data.enumerated().map { i, v in
                CGPoint(
                    x: padL + CGFloat(i) * stepX,
                    y: padT + h - (CGFloat(v) / maxV) * h)
            }

            // Baseline.
            var baseline = Path()
            baseline.move(to: CGPoint(x: padL, y: padT + h))
            baseline.addLine(to: CGPoint(x: size.width - padR, y: padT + h))
            ctx.stroke(baseline, with: .color(themeStore.dividerColor()), lineWidth: 1)

            // Trend line.
            var line = Path()
            line.addLines(pts)
            ctx.stroke(
                line, with: .color(themeStore.accentColor()),
                style: StrokeStyle(lineWidth: 1.6, lineCap: .round, lineJoin: .round))

            // Dots, last one emphasized.
            let accent = themeStore.accentColor()
            let bg = themeStore.commandModeBackgroundColor()
            for (i, p) in pts.enumerated() {
                let last = i == pts.count - 1
                let r: CGFloat = last ? 3 : 1.5
                let rect = CGRect(x: p.x - r, y: p.y - r, width: r * 2, height: r * 2)
                if last {
                    ctx.fill(Path(ellipseIn: rect), with: .color(accent))
                } else {
                    ctx.fill(Path(ellipseIn: rect), with: .color(bg))
                    ctx.stroke(Path(ellipseIn: rect), with: .color(accent), lineWidth: 1.2)
                }
            }
        }
    }
}

struct TodoHeatmap: View {
    let columns: [[TodoHeatDay]]
    let themeStore: ThemeStore
    var cell: CGFloat = 12
    var gap: CGFloat = 3

    private let dayLabels = ["", "M", "", "W", "", "F", ""]
    private let labelColWidth: CGFloat = 12
    private let labelSpacing: CGFloat = 5
    private var gridHeight: CGFloat { cell * 7 + gap * 6 }

    var body: some View {
        // Render only the most recent weeks that fit the available width,
        // so the grid never clips on the right regardless of screen size.
        GeometryReader { geo in
            let available = geo.size.width - labelColWidth - labelSpacing
            let fit = max(1, Int((available + gap) / (cell + gap)))
            let shown = Array(columns.suffix(min(fit, columns.count)))

            HStack(alignment: .top, spacing: labelSpacing) {
                VStack(spacing: gap) {
                    ForEach(0..<7, id: \.self) { i in
                        Text(dayLabels[i])
                            .font(.system(size: 8, design: .monospaced))
                            .foregroundStyle(themeStore.mutedTextColor())
                            .frame(width: labelColWidth, height: cell, alignment: .leading)
                    }
                }
                HStack(spacing: gap) {
                    ForEach(Array(shown.enumerated()), id: \.offset) { _, week in
                        VStack(spacing: gap) {
                            ForEach(week) { day in
                                RoundedRectangle(cornerRadius: 3, style: .continuous)
                                    .fill(TodoHeatColors.color(day.level, themeStore: themeStore))
                                    .frame(width: cell, height: cell)
                                    .hoverTooltip(tooltip(day), width: 160, edge: .top)
                            }
                        }
                    }
                }
                Spacer(minLength: 0)
            }
        }
        .frame(height: gridHeight)
    }

    private func tooltip(_ day: TodoHeatDay) -> String {
        let date = TodoAnalytics.heatDateFormatter.string(from: day.date)
        return day.hasTasks ? "\(date): \(day.done)/\(day.total) done" : "\(date): no tasks"
    }
}

struct TodoHeatLegend: View {
    let themeStore: ThemeStore

    var body: some View {
        HStack(spacing: 5) {
            Text("Less")
            ForEach(0..<5, id: \.self) { l in
                RoundedRectangle(cornerRadius: 2, style: .continuous)
                    .fill(TodoHeatColors.color(l, themeStore: themeStore))
                    .frame(width: 10, height: 10)
            }
            Text("More")
        }
        .font(.system(size: 10, design: .monospaced))
        .foregroundStyle(themeStore.mutedTextColor())
    }
}

/// Stepped intensity ramp derived from the theme accent so the heatmap
/// stays theme-aware (level 0 is a faint recess, 1 through 4 ramp up the
/// accent).
enum TodoHeatColors {
    static func color(_ level: Int, themeStore: ThemeStore) -> Color {
        switch level {
        case 1: return themeStore.accentColor().opacity(0.28)
        case 2: return themeStore.accentColor().opacity(0.50)
        case 3: return themeStore.accentColor().opacity(0.74)
        case 4: return themeStore.accentColor()
        default: return themeStore.mutedTextColor().opacity(0.12)
        }
    }
}
