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
    /// When the task was created, for backend round-tripping.
    var createdAtUnixS: Int64 = Int64(Date().timeIntervalSince1970)

    static func newID() -> String {
        "n" + UUID().uuidString.prefix(6).lowercased()
    }
}

/// Flat task shape exchanged with the shared core backend (matches
/// `look_todo::TodoTask`'s JSON). The app groups these by `dueDate`.
struct TodoBackendTask: Codable {
    let id: String
    let name: String
    let done: Bool
    let dueDate: String
    let createdAtUnixS: Int64

    enum CodingKeys: String, CodingKey {
        case id, name, done
        case dueDate = "due_date"
        case createdAtUnixS = "created_at_unix_s"
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
    // Produces the due_date keys stored in the shared database, so it is
    // pinned to POSIX/Gregorian: a system set to a non-Gregorian calendar
    // (e.g. Buddhist) must not write "2569-07-05" into a cross-platform
    // store that the Rust retention prune compares against Gregorian
    // date('now'). Timezone stays local so "today" is the user's day.
    static let keyFormatter: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.calendar = Calendar(identifier: .gregorian)
        f.dateFormat = "yyyy-MM-dd"
        return f
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
    /// How many past days the Tasks list shows when not searching. The
    /// full retained year stays searchable and feeds analytics; the
    /// browse list only surfaces the recent window.
    static let listWindowDays = 31

    /// Case- and diacritic-insensitive subsequence match, ignoring
    /// whitespace in the query: "jul3" matches "Jul 3", and "di" matches
    /// "đi" (Vietnamese and other accented text folds to ASCII).
    static func fuzzyMatch(_ query: String, _ target: String) -> Bool {
        let needle = searchNormalize(query).filter { !$0.isWhitespace }
        guard !needle.isEmpty else { return true }
        let hay = searchNormalize(target)
        var ni = needle.startIndex
        for ch in hay where ch == needle[ni] {
            ni = needle.index(after: ni)
            if ni == needle.endIndex { return true }
        }
        return false
    }

    /// Swift mirror of the engine's `normalize_for_search`
    /// (core/engine/src/normalize.rs), which is what lets "tẻ" find
    /// Terminal in the app list: fold case/diacritics/width, then map
    /// the Vietnamese đ/Đ, a stroke letter that no diacritic fold
    /// touches, to plain d. Keep the two in sync.
    private static func searchNormalize(_ text: String) -> String {
        text.folding(
            options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive], locale: nil
        )
        .replacingOccurrences(of: "đ", with: "d")
        .replacingOccurrences(of: "Đ", with: "d")
    }
}

@Observable
final class TodoState {
    private(set) var groups: [TodoGroup]
    /// Unsaved edits pending. Save persists them to the backend.
    var dirty: Bool = false

    /// Number of `future` placeholder days already generated, so
    /// "Add date" walks forward one calendar day at a time.
    @ObservationIgnored private var futureDaysAdded = 0

    init() {
        groups = TodoPersistence.load()
        ensureTodayGroup()
    }

    /// Keeps an empty Today group present. Groups derive from stored
    /// tasks, so an empty store would leave nowhere to type. Also re-run
    /// when the panel appears, so an app left resident across midnight
    /// gets a fresh Today without a relaunch. Not a user edit, so it
    /// deliberately does not mark the state dirty.
    func ensureTodayGroup() {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let key = TodoGroup.keyFormatter.string(from: today)
        guard !groups.contains(where: { $0.key == key }) else { return }
        groups.append(TodoGroup(key: key, date: today, tasks: []))
        groups.sort { $0.date > $1.date }
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
        TodoPersistence.save(groups)
        dirty = false
    }
}

/// Kept in a static so edits survive the launcher window hiding (the
/// WindowGroup retains its view tree while hidden), and so the hint-bar
/// quick view can read today's counts later.
enum TodoSharedState {
    @MainActor static let shared = TodoState()
}

// Bridges the panel's grouped model to the shared core backend (via
// EngineBridge → look_todo). Load reads the flat task set and groups it
// by date; save flattens the groups back and replaces the stored set.
enum TodoPersistence {
    static func load() -> [TodoGroup] {
        groups(from: EngineBridge.shared.todoList())
    }

    static func save(_ groups: [TodoGroup]) {
        EngineBridge.shared.todoSave(backendTasks(from: groups))
    }

    private static func groups(from tasks: [TodoBackendTask]) -> [TodoGroup] {
        let cal = Calendar.current
        var byDate: [String: [TodoTask]] = [:]
        for task in tasks {
            byDate[task.dueDate, default: []].append(
                TodoTask(
                    id: task.id, name: task.name, done: task.done,
                    createdAtUnixS: task.createdAtUnixS))
        }
        var out: [TodoGroup] = []
        for (key, list) in byDate {
            guard let date = TodoGroup.keyFormatter.date(from: key) else { continue }
            out.append(TodoGroup(key: key, date: cal.startOfDay(for: date), tasks: list))
        }
        out.sort { $0.date > $1.date }
        return out
    }

    private static func backendTasks(from groups: [TodoGroup]) -> [TodoBackendTask] {
        groups.flatMap { group in
            group.tasks.map { task in
                TodoBackendTask(
                    id: task.id, name: task.name, done: task.done,
                    dueDate: group.key, createdAtUnixS: task.createdAtUnixS)
            }
        }
    }
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

// Real aggregates over the loaded task set. The client holds the full
// retained year in memory, so every chart derives from `groups` and
// updates live as tasks change.

enum TodoAnalytics {
    /// (done, total) per start-of-day.
    static func dayCounts(_ groups: [TodoGroup]) -> [Date: (done: Int, total: Int)] {
        var out: [Date: (done: Int, total: Int)] = [:]
        for g in groups where !g.tasks.isEmpty {
            let prior = out[g.date] ?? (0, 0)
            out[g.date] = (prior.done + g.doneCount, prior.total + g.total)
        }
        return out
    }

    /// Done/total for the calendar period containing today (.weekOfYear
    /// for the week card, .month for the month card).
    static func stat(_ groups: [TodoGroup], sameAs component: Calendar.Component) -> TodoStat {
        let cal = Calendar.current
        let now = Date()
        var done = 0
        var total = 0
        for g in groups where cal.isDate(g.date, equalTo: now, toGranularity: component) {
            done += g.doneCount
            total += g.total
        }
        return TodoStat(done: done, total: total)
    }

    /// Done-count per day for the last 30 days, oldest first.
    static func monthTrend(_ groups: [TodoGroup]) -> [Int] {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let counts = dayCounts(groups)
        var arr: [Int] = []
        for offset in -29...0 {
            guard let date = cal.date(byAdding: .day, value: offset, to: today) else { continue }
            arr.append(counts[date]?.done ?? 0)
        }
        return arr
    }

    /// Consecutive days with at least one completed task, counting back
    /// from today. A doneless today doesn't break the streak until the
    /// day is actually over, so the walk starts at yesterday and today
    /// only extends it.
    static func streakDays(_ groups: [TodoGroup]) -> Int {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let counts = dayCounts(groups)
        var streak = (counts[today]?.done ?? 0) > 0 ? 1 : 0
        var cursor = today
        while let prev = cal.date(byAdding: .day, value: -1, to: cursor),
            (counts[prev]?.done ?? 0) > 0
        {
            streak += 1
            cursor = prev
        }
        return streak
    }

    /// A year of activity as GitHub-style week columns (each column is a
    /// Sun...Sat week; the last column contains today). Days after today
    /// in the current week are empty placeholders.
    static let heatmapWeekCount = 52

    static func heatmapDays(_ groups: [TodoGroup]) -> [[TodoHeatDay]] {
        let cal = Calendar.current
        let today = cal.startOfDay(for: Date())
        let daysSinceSunday = cal.component(.weekday, from: today) - 1
        guard let lastSunday = cal.date(byAdding: .day, value: -daysSinceSunday, to: today)
        else { return [] }

        let counts = dayCounts(groups)
        var columns: [[TodoHeatDay]] = []
        for w in stride(from: heatmapWeekCount - 1, through: 0, by: -1) {
            guard let colSunday = cal.date(byAdding: .day, value: -7 * w, to: lastSunday)
            else { continue }
            var col: [TodoHeatDay] = []
            for d in 0..<7 {
                guard let date = cal.date(byAdding: .day, value: d, to: colSunday) else { continue }
                let day = date > today ? (done: 0, total: 0) : (counts[date] ?? (done: 0, total: 0))
                col.append(TodoHeatDay(date: date, done: day.done, total: day.total))
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

    static let axisDateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "MMM d"
        return f
    }()
}
