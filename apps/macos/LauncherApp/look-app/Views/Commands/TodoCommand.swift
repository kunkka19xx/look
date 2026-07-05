import Foundation
import Observation

// TodoState is the source of truth for the panel, mirroring how
// PomoState works for /pomo.
//
// State runs on in-memory data. There is no database yet: TodoState
// seeds a fresh set of groups on launch and Save just clears the dirty
// flag. TodoPersistence is the storage seam; wiring real one-year
// retention (ideally in core/ so linows can reuse it) is a change to
// that type alone, with TodoState untouched.

// A task is two-state (todo / done). The spec considered an in-progress
// state but dropped it, so completion is a plain `done` flag.
struct TodoTask: Identifiable, Equatable {
    let id: String
    var name: String
    var done: Bool

    static func newID() -> String {
        "n" + UUID().uuidString.prefix(6).lowercased()
    }
}

/// A date bucket relative to today. `today`/`future` are editable;
/// `past` days are read-mostly (names still editable, but no bulk
/// complete/clear and no adding tasks).
enum TodoDayKind {
    case past
    case today
    case future
}

struct TodoGroup: Identifiable, Equatable {
    /// ISO `yyyy-MM-dd`, also used as the stable identity + sort key.
    let key: String
    let date: Date
    var tasks: [TodoTask]

    var id: String { key }

    var kind: TodoDayKind {
        let cal = Calendar.current
        if cal.isDateInToday(date) { return .today }
        return date < cal.startOfDay(for: Date()) ? .past : .future
    }

    var doneCount: Int { tasks.filter(\.done).count }
    var total: Int { tasks.count }
    /// Unfinished (todo) tasks. This is what the per-day limit caps;
    /// total tasks are unlimited.
    var openCount: Int { total - doneCount }

    /// Short weekday, e.g. "Sat". "Today" is rendered by the header,
    /// not here.
    var weekday: String {
        Self.weekdayFormatter.string(from: date)
    }

    /// e.g. "Jul 5".
    var monthDay: String {
        Self.monthDayFormatter.string(from: date)
    }

    /// Relative phrase like "Today", "Tomorrow", "Yesterday", "In 3
    /// days". Empty when the day is far enough away that a relative
    /// phrase reads oddly (the month/day already communicates it).
    var relative: String {
        let cal = Calendar.current
        let days = cal.dateComponents([.day],
            from: cal.startOfDay(for: Date()),
            to: cal.startOfDay(for: date)).day ?? 0
        switch days {
        case 0: return "Today"
        case 1: return "Tomorrow"
        case -1: return "Yesterday"
        case 2...6: return "In \(days) days"
        default: return ""
        }
    }

    private static let weekdayFormatter: DateFormatter = {
        let f = DateFormatter(); f.dateFormat = "EEE"; return f
    }()
    private static let monthDayFormatter: DateFormatter = {
        let f = DateFormatter(); f.dateFormat = "MMM d"; return f
    }()
    static let keyFormatter: DateFormatter = {
        let f = DateFormatter(); f.dateFormat = "yyyy-MM-dd"; return f
    }()
}

struct TodoStat: Equatable {
    var done: Int
    var total: Int
    var fraction: Double { total > 0 ? Double(done) / Double(total) : 0 }
}

enum TodoCommand {
    /// Max unfinished (todo) tasks per day. Completing a task frees a
    /// slot; total tasks per day are unlimited. Spec allows 3 or 5.
    static let taskLimit = 3
    /// Max upcoming (future) date groups the user can add ahead.
    static let dateGroupLimit = 3
    /// Max characters in a task name. Clamped at the input field and
    /// truncated in the model as a backstop.
    static let taskNameMaxLength = 256

    /// Case-insensitive subsequence match, ignoring whitespace in the
    /// query, so "jul3", "jul 3", and "j3" all match "Jul 3".
    static func fuzzyMatch(_ query: String, _ target: String) -> Bool {
        let needle = query.lowercased().filter { !$0.isWhitespace }
        guard !needle.isEmpty else { return true }
        let hay = target.lowercased()
        var ni = needle.startIndex
        for ch in hay where ch == needle[ni] {
            ni = needle.index(after: ni)
            if ni == needle.endIndex { return true }
        }
        return false
    }
}

@Observable
final class TodoState {
    private(set) var groups: [TodoGroup]
    /// Unsaved edits pending. Save clears it (no DB yet).
    var dirty: Bool = false

    /// Number of `future` placeholder days already generated, so
    /// "Add date" walks forward one calendar day at a time.
    @ObservationIgnored private var futureDaysAdded = 0

    init() {
        groups = TodoState.seed()
    }

    var today: TodoGroup? { groups.first { $0.kind == .today } }

    var todayStat: TodoStat {
        guard let t = today else { return TodoStat(done: 0, total: 0) }
        return TodoStat(done: t.doneCount, total: t.total)
    }

    var futureCount: Int { groups.filter { $0.kind == .future }.count }
    var canAddDateGroup: Bool { futureCount < TodoCommand.dateGroupLimit }
    var groupsLeft: Int { max(0, TodoCommand.dateGroupLimit - futureCount) }

    private func mutate(_ body: (inout [TodoGroup]) -> Void) {
        body(&groups)
        dirty = true
    }

    private func withGroup(_ key: String, _ body: (inout TodoGroup) -> Void) {
        mutate { gs in
            guard let i = gs.firstIndex(where: { $0.key == key }) else { return }
            body(&gs[i])
        }
    }

    func toggleTask(group key: String, task id: String) {
        withGroup(key) { g in
            guard let i = g.tasks.firstIndex(where: { $0.id == id }) else { return }
            g.tasks[i].done.toggle()
        }
    }

    func removeTask(group key: String, task id: String) {
        withGroup(key) { g in g.tasks.removeAll { $0.id == id } }
    }

    func editTask(group key: String, task id: String, name: String) {
        let trimmed = Self.clampName(name)
        guard !trimmed.isEmpty else { return }
        withGroup(key) { g in
            guard let i = g.tasks.firstIndex(where: { $0.id == id }) else { return }
            g.tasks[i].name = trimmed
        }
    }

    /// Trims whitespace and caps length at `taskNameMaxLength`.
    static func clampName(_ name: String) -> String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        return String(trimmed.prefix(TodoCommand.taskNameMaxLength))
    }

    func completeAll(group key: String) {
        withGroup(key) { g in
            for i in g.tasks.indices { g.tasks[i].done = true }
        }
    }

    func clearAll(group key: String) {
        withGroup(key) { g in g.tasks.removeAll() }
    }

    /// Adds a task, respecting the per-day limit on unfinished tasks.
    /// Returns false when the day is at its open-task limit so the caller
    /// can leave the field intact.
    @discardableResult
    func addTask(group key: String, name: String) -> Bool {
        let trimmed = Self.clampName(name)
        guard !trimmed.isEmpty else { return false }
        guard let g = groups.first(where: { $0.key == key }),
              g.openCount < TodoCommand.taskLimit else { return false }
        withGroup(key) { g in
            g.tasks.append(TodoTask(id: TodoTask.newID(), name: trimmed, done: false))
        }
        return true
    }

    /// Adds the next future day group (tomorrow, then the day after,
    /// etc.), up to `dateGroupLimit` upcoming groups.
    func addDateGroup() {
        guard canAddDateGroup else { return }
        let cal = Calendar.current
        // Walk forward until we hit a day not already present.
        var offset = futureDaysAdded + 1
        while offset < 60 {
            guard let date = cal.date(byAdding: .day, value: offset, to: cal.startOfDay(for: Date())) else { return }
            let key = TodoGroup.keyFormatter.string(from: date)
            if !groups.contains(where: { $0.key == key }) {
                mutate { gs in
                    gs.append(TodoGroup(key: key, date: date, tasks: []))
                    gs.sort { $0.date > $1.date }   // latest on top
                }
                futureDaysAdded = offset
                return
            }
            offset += 1
        }
    }

    func save() {
        // TODO(todo-db): persist `groups` to core storage with one-year
        // retention. For now, saving just acknowledges the edits.
        TodoPersistence.save(groups)
        dirty = false
    }

    // Sample content anchored to the real current date (today, plus two
    // future and three past days) so the panel is usable before storage
    // exists. Replaced by TodoPersistence.load() once that returns data.

    static func seed() -> [TodoGroup] {
        if let restored = TodoPersistence.load() { return restored }

        let cal = Calendar.current
        let base = cal.startOfDay(for: Date())

        // (dayOffset, [(name, done)])
        let plan: [(Int, [(String, Bool)])] = [
            ( 2, [("Prep sprint demo slides", false),
                  ("Book dentist appointment", false)]),
            ( 1, [("Grocery run + meal prep", false)]),
            ( 0, [("Ship running-apps switcher PR", true),
                  ("Review Todo command spec", true),
                  ("Reply to design feedback thread", false)]),
            (-1, [("Merge theme presets", true),
                  ("Fix hotkey race condition", true),
                  ("Write bench notes", true)]),
            (-2, [("Wire AI answer card", true),
                  ("Add Rose Pine theme", true),
                  ("QuickLook preview fix", true)]),
            (-3, [("Single-instance lock", true),
                  ("Update checker polling", true),
                  ("Draft user guide section", false)]),
        ]

        var out: [TodoGroup] = []
        for (offset, items) in plan {
            guard let date = cal.date(byAdding: .day, value: offset, to: base) else { continue }
            let key = TodoGroup.keyFormatter.string(from: date)
            let tasks = items.map { TodoTask(id: TodoTask.newID(), name: $0.0, done: $0.1) }
            out.append(TodoGroup(key: key, date: date, tasks: tasks))
        }
        out.sort { $0.date > $1.date }   // latest date on top, desc
        return out
    }
}

/// Kept in a static so edits survive the launcher window hiding (the
/// WindowGroup retains its view tree while hidden), and so the hint-bar
/// quick view can read today's counts later.
enum TodoSharedState {
    @MainActor static let shared = TodoState()
}

// Intentionally a no-op today. Kept as a named seam so wiring real
// storage later is a one-file change and TodoState needs no edits.

enum TodoPersistence {
    static func load() -> [TodoGroup]? { nil }
    static func save(_ groups: [TodoGroup]) { /* no DB yet */ }
}

/// One day cell in the activity heatmap: a real date with that day's
/// done/total counts. `level` buckets `done` into the color ramp.
struct TodoHeatDay: Identifiable, Equatable {
    let date: Date
    let done: Int
    let total: Int

    var id: Date { date }
    var hasTasks: Bool { total > 0 }
    var level: Int {
        switch done {
        case 0: return 0
        case 1: return 1
        case 2: return 3
        default: return 4
        }
    }
}

// Deterministic seeded series so the charts render a stable shape.
// Placeholder aggregates until storage exists.

enum TodoAnalytics {
    static let week = TodoStat(done: 12, total: 18)
    static let month = TodoStat(done: 47, total: 62)
    static let streakDays = 6

    /// Deterministic linear congruential generator.
    private struct Seeded {
        var s: Int
        init(_ n: Int) { s = n &* 9301 &+ 49297 }
        mutating func next() -> Double {
            s = (s &* 9301 &+ 49297) % 233_280
            return Double(s) / 233_280.0
        }
    }

    /// Last 30 days, done-count per day, capped at the daily task limit.
    static func monthTrend() -> [Int] {
        let cap = TodoCommand.taskLimit
        var r = Seeded(7)
        var arr: [Int] = []
        for i in 0..<30 {
            let base = sin(Double(i) / 4) * Double(cap) * 0.45 + Double(cap) * 0.65
            let v = base + (r.next() - 0.5) * Double(cap) * 0.9
            arr.append(max(0, min(cap, Int(v.rounded()))))
        }
        return arr
    }

    /// A year of activity as GitHub-style week columns (each column is a
    /// Sun...Sat week; the last column contains today). Days after today
    /// in the current week are empty placeholders.
    static let heatmapWeekCount = 52

    static func heatmapDays() -> [[TodoHeatDay]] {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let daysSinceSunday = cal.component(.weekday, from: today) - 1
        guard let lastSunday = cal.date(byAdding: .day, value: -daysSinceSunday, to: today)
        else { return [] }

        var r = Seeded(21)
        var columns: [[TodoHeatDay]] = []
        for w in stride(from: heatmapWeekCount - 1, through: 0, by: -1) {
            guard let colSunday = cal.date(byAdding: .day, value: -7 * w, to: lastSunday)
            else { continue }
            var col: [TodoHeatDay] = []
            for d in 0..<7 {
                guard let date = cal.date(byAdding: .day, value: d, to: colSunday) else { continue }
                if date > today {
                    col.append(TodoHeatDay(date: date, done: 0, total: 0))
                    continue
                }
                let v = r.next()
                let done = v > 0.82 ? 3 : (v > 0.60 ? 2 : (v > 0.36 ? 1 : 0))
                let total = done + (r.next() > 0.55 ? 1 : 0)
                col.append(TodoHeatDay(date: date, done: done, total: total))
            }
            columns.append(col)
        }
        return columns
    }

    static let heatDateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "EEE, MMM d"
        return f
    }()
}
