import SwiftUI

// Week / month done-total donuts, a current streak, a 30-day completion
// trend line, and a GitHub-style activity heatmap. Series come from
// TodoAnalytics (deterministic placeholders until storage exists).

struct TodoAnalyticsPage: View {
    let themeStore: ThemeStore

    private let trend = TodoAnalytics.monthTrend()
    private let weeks = TodoAnalytics.heatmapWeeks()

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                TodoStatStrip(
                    themeStore: themeStore,
                    week: TodoAnalytics.week,
                    month: TodoAnalytics.month,
                    streak: TodoAnalytics.streakDays
                )

                VStack(alignment: .leading, spacing: 8) {
                    sectionLabel("chart.bar", "Completion trend · 30 days")
                    VStack(spacing: 4) {
                        TodoLineChart(data: trend, themeStore: themeStore)
                            .frame(height: 92)
                        HStack {
                            Text("Jun 5"); Spacer(); Text("Jun 20"); Spacer(); Text("Jul 4")
                        }
                        .font(.system(size: 9.5, design: .monospaced))
                        .foregroundStyle(themeStore.mutedTextColor())
                    }
                    .padding(.horizontal, 12)
                    .padding(.top, 10)
                    .padding(.bottom, 6)
                    .todoCard(themeStore)
                }

                VStack(alignment: .leading, spacing: 8) {
                    HStack(alignment: .center) {
                        sectionLabel("calendar", "Activity · last 18 weeks")
                        Spacer()
                        TodoHeatLegend(themeStore: themeStore)
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        TodoHeatmap(weeks: weeks, themeStore: themeStore)
                    }
                    .padding(12)
                    .todoCard(themeStore)
                }

                VStack(alignment: .leading, spacing: 8) {
                    sectionLabel("sparkles", "Insights · last 30 days (Tasks)")
                    TodoInsightsStrip(themeStore: themeStore, trend: trend)
                }
            }
            .padding(.horizontal, 4)
            .padding(.vertical, 2)
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
            tile("Avg / day", avgPerDay, help: "Average tasks completed per day over the last 30 days")
            divider
            tile("Best day", "\(bestDay)", help: "Most tasks completed in a single day")
            divider
            tile("Active days", "\(activeDays)/\(trend.count)", help: "Days with at least one task completed")
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
        HStack(spacing: 4) {
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

struct TodoMetricColumn: View {
    let themeStore: ThemeStore
    let label: String
    let stat: TodoStat

    var body: some View {
        HStack(spacing: 12) {
            TodoDonut(fraction: stat.fraction, themeStore: themeStore)
            VStack(alignment: .leading, spacing: 3) {
                Text(label.uppercased())
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(0.7)
                    .foregroundStyle(themeStore.mutedTextColor())
                HStack(alignment: .firstTextBaseline, spacing: 3) {
                    Text("\(stat.done)")
                        .font(themeStore.uiFont(size: 22, weight: .bold))
                        .foregroundStyle(themeStore.fontColor())
                    Text("/ \(stat.total)")
                        .font(themeStore.uiFont(size: 13))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity)
    }
}

struct TodoStreakColumn: View {
    let themeStore: ThemeStore
    let days: Int

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "flame")
                .font(.system(size: 24, weight: .light))
                .foregroundStyle(themeStore.accentColor())
                .frame(width: 46, height: 46)
            VStack(alignment: .leading, spacing: 5) {
                Text("STREAK")
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(0.7)
                    .foregroundStyle(themeStore.mutedTextColor())
                HStack(alignment: .firstTextBaseline, spacing: 4) {
                    Text("\(days)")
                        .font(themeStore.uiFont(size: 22, weight: .bold))
                        .foregroundStyle(themeStore.fontColor())
                    Text("days")
                        .font(themeStore.uiFont(size: 12.5))
                        .foregroundStyle(themeStore.secondaryTextColor())
                }
                HStack(spacing: 4) {
                    let filled = min(days, 7)
                    ForEach(0..<7, id: \.self) { i in
                        let on = i >= 7 - filled
                        Circle()
                            .fill(on ? themeStore.accentColor() : Color.clear)
                            .overlay(Circle().stroke(on ? Color.clear : themeStore.dividerColor(), lineWidth: 1))
                            .frame(width: 6, height: 6)
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity)
    }
}

struct TodoDonut: View {
    let fraction: Double
    let themeStore: ThemeStore
    var size: CGFloat = 46
    var stroke: CGFloat = 4.5

    var body: some View {
        ZStack {
            Circle().stroke(themeStore.dividerColor(), lineWidth: stroke)
            Circle()
                .trim(from: 0, to: max(0, min(1, fraction)))
                .stroke(themeStore.accentColor(), style: StrokeStyle(lineWidth: stroke, lineCap: .round))
                .rotationEffect(.degrees(-90))
            Text("\(Int((fraction * 100).rounded()))")
                .font(themeStore.uiFont(size: size * 0.28, weight: .bold))
                .foregroundStyle(themeStore.fontColor())
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
            let padL: CGFloat = 4, padR: CGFloat = 4, padT: CGFloat = 10, padB: CGFloat = 8
            let w = size.width - padL - padR
            let h = size.height - padT - padB
            let maxV = CGFloat(max(TodoCommand.taskLimit, data.max() ?? TodoCommand.taskLimit))
            let stepX = w / CGFloat(data.count - 1)

            let pts: [CGPoint] = data.enumerated().map { i, v in
                CGPoint(x: padL + CGFloat(i) * stepX,
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
            ctx.stroke(line, with: .color(themeStore.accentColor()),
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
    let weeks: [[Int]]
    let themeStore: ThemeStore
    var cell: CGFloat = 12
    var gap: CGFloat = 3

    private let dayLabels = ["", "M", "", "W", "", "F", ""]

    var body: some View {
        HStack(alignment: .top, spacing: 5) {
            VStack(spacing: gap) {
                ForEach(0..<7, id: \.self) { i in
                    Text(dayLabels[i])
                        .font(.system(size: 8, design: .monospaced))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .frame(width: 10, height: cell, alignment: .leading)
                }
            }
            HStack(spacing: gap) {
                ForEach(Array(weeks.enumerated()), id: \.offset) { _, week in
                    VStack(spacing: gap) {
                        ForEach(Array(week.enumerated()), id: \.offset) { _, level in
                            RoundedRectangle(cornerRadius: 3, style: .continuous)
                                .fill(TodoHeatColors.color(level, themeStore: themeStore))
                                .frame(width: cell, height: cell)
                        }
                    }
                }
            }
        }
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
