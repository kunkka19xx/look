//! The day planner's data, kept for the process: the tasks by day, the undo
//! and redo stacks and what Save last wrote, since the window is rebuilt on
//! every summon and an unsaved edit must not go with it. The rules are the
//! webview's (`screens/commands/todo.js`), which are macOS's: three
//! unfinished tasks a day at most, three upcoming days, a three-day window
//! in which a late task can still be ticked. Nothing here saves by itself;
//! the panel writes the whole set back on Save.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use chrono::{Datelike, Local, NaiveDate};
use linows_backend::look_matching::normalize_for_search;
use linows_backend::look_todo::TodoTask;

pub const UNFINISHED_LIMIT: usize = 3;
pub const FUTURE_GROUP_LIMIT: usize = 3;
/// Days late an unfinished task stays completable before it locks.
pub const EXTENSION_WINDOW_DAYS: i64 = 3;
pub const TREND_DAYS: usize = 30;
pub const HEATMAP_WEEKS: usize = 52;
const UNDO_LIMIT: usize = 50;
/// Browsing shows this many past days; a search spans the retained year.
const BROWSE_PAST_DAYS: i64 = 31;
const NAME_MAX: usize = 256;
/// The store's day key, the one macOS writes.
const DAY_KEY: &str = "%Y-%m-%d";
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub done: bool,
    created_at: i64,
}

pub type Days = BTreeMap<NaiveDate, Vec<Task>>;

#[derive(Clone)]
struct Snapshot {
    days: Days,
    revision: u64,
}

pub struct Todo {
    days: Days,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    revision: u64,
    next_revision: u64,
    saved_revision: u64,
    pub loaded: bool,
}

static STATE: Mutex<Todo> = Mutex::new(Todo {
    days: BTreeMap::new(),
    undo: Vec::new(),
    redo: Vec::new(),
    revision: 0,
    next_revision: 0,
    saved_revision: 0,
    loaded: false,
});

pub fn lock() -> MutexGuard<'static, Todo> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// `date` relative to today: 0 today, positive ahead, negative behind.
pub fn day_diff(date: NaiveDate) -> i64 {
    (date - today()).num_days()
}

/// Today, Tomorrow, Yesterday, In N days up to a week out; nothing further.
pub fn relative_phrase(diff: i64) -> Option<String> {
    match diff {
        0 => Some("Today".into()),
        1 => Some("Tomorrow".into()),
        -1 => Some("Yesterday".into()),
        2..=6 => Some(format!("In {diff} days")),
        _ => None,
    }
}

pub fn weekday(date: NaiveDate) -> &'static str {
    WEEKDAYS[date.weekday().num_days_from_sunday() as usize]
}

/// `Oct 6`.
pub fn month_day(date: NaiveDate) -> String {
    format!("{} {}", MONTHS[date.month0() as usize], date.day())
}

fn now_unix_s() -> i64 {
    Local::now().timestamp()
}

/// The search's folding: case and diacritics dropped, whitespace too.
pub fn fold_query(text: &str) -> String {
    normalize_for_search(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// A folded `needle` as a subsequence of `target`: `jul3` finds Jul 3,
/// `di` finds `đi`.
pub fn subsequence_match(needle: &str, target: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut wanted = needle.chars();
    let mut next = wanted.next();
    for ch in normalize_for_search(target).chars() {
        if Some(ch) == next {
            next = wanted.next();
            if next.is_none() {
                return true;
            }
        }
    }
    false
}

/// What a day card's title says, for the search to match on.
pub fn date_search_text(date: NaiveDate, diff: i64) -> String {
    format!(
        "{} {} {}",
        weekday(date),
        month_day(date),
        relative_phrase(diff).unwrap_or_default()
    )
}

/// Done and total per day.
pub type Counts = BTreeMap<NaiveDate, (usize, usize)>;

pub enum Period {
    Week,
    Month,
}

impl Todo {
    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn days(&self) -> &Days {
        &self.days
    }

    /// The store's rows, unless edits are waiting on Save: a read that
    /// answers late must not clobber them.
    pub fn replace(&mut self, rows: Vec<TodoTask>) {
        if self.is_dirty() {
            return;
        }
        self.days = BTreeMap::new();
        for row in rows {
            let Some(day) = NaiveDate::parse_from_str(&row.due_date, DAY_KEY).ok() else {
                continue;
            };
            self.days.entry(day).or_default().push(Task {
                id: row.id,
                name: row.name,
                done: row.done,
                created_at: row.created_at_unix_s,
            });
        }
        self.loaded = true;
        self.undo.clear();
        self.redo.clear();
        self.next_revision += 1;
        self.revision = self.next_revision;
        self.saved_revision = self.revision;
        self.ensure_today();
    }

    /// Everything, in the store's shape, with the revision it stands for.
    pub fn for_save(&self) -> (Vec<TodoTask>, u64) {
        let tasks = self
            .days
            .iter()
            .flat_map(|(day, list)| {
                let due = day.format(DAY_KEY).to_string();
                list.iter().map(move |t| TodoTask {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    done: t.done,
                    due_date: due.clone(),
                    created_at_unix_s: t.created_at,
                })
            })
            .collect();
        (tasks, self.revision)
    }

    /// A save landed: history survives it, so undoing back past the saved
    /// point relights Save by itself.
    pub fn mark_saved(&mut self, revision: u64) {
        self.saved_revision = revision;
    }

    pub fn ensure_today(&mut self) {
        self.days.entry(today()).or_default();
    }

    /// `affects_save` false for an empty date placeholder: undoable, but no
    /// revision bump.
    fn remember(&mut self, affects_save: bool) {
        self.redo.clear();
        self.undo.push(Snapshot {
            days: self.days.clone(),
            revision: self.revision,
        });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        if affects_save {
            self.next_revision += 1;
            self.revision = self.next_revision;
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = Snapshot {
            days: std::mem::replace(&mut self.days, snapshot.days),
            revision: self.revision,
        };
        self.redo.push(current);
        self.revision = snapshot.revision;
        self.ensure_today();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = Snapshot {
            days: std::mem::replace(&mut self.days, snapshot.days),
            revision: self.revision,
        };
        self.undo.push(current);
        self.revision = snapshot.revision;
        self.ensure_today();
        true
    }

    pub fn open_count(&self, day: NaiveDate) -> usize {
        self.days
            .get(&day)
            .map_or(0, |list| list.iter().filter(|t| !t.done).count())
    }

    /// True when the task went in; a blank name or a full day refuses.
    pub fn add_task(&mut self, day: NaiveDate, raw: &str) -> bool {
        let name: String = raw.trim().chars().take(NAME_MAX).collect();
        if name.is_empty() || self.open_count(day) >= UNFINISHED_LIMIT {
            return false;
        }
        self.remember(true);
        self.days.entry(day).or_default().push(Task {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            done: false,
            created_at: now_unix_s(),
        });
        true
    }

    /// Ticked or unticked, unless the day is past the extension window.
    pub fn toggle(&mut self, day: NaiveDate, id: &str) {
        if -day_diff(day) > EXTENSION_WINDOW_DAYS {
            return;
        }
        let Some(index) = self.index_of(day, id) else {
            return;
        };
        self.remember(true);
        let task = &mut self.days.get_mut(&day).expect("day exists")[index];
        task.done = !task.done;
    }

    pub fn rename(&mut self, day: NaiveDate, id: &str, raw: &str) {
        let name: String = raw.trim().chars().take(NAME_MAX).collect();
        let Some(index) = self.index_of(day, id) else {
            return;
        };
        if name.is_empty() || self.days[&day][index].name == name {
            return;
        }
        self.remember(true);
        self.days.get_mut(&day).expect("day exists")[index].name = name;
    }

    pub fn remove(&mut self, day: NaiveDate, id: &str) {
        let Some(index) = self.index_of(day, id) else {
            return;
        };
        self.remember(true);
        let list = self.days.get_mut(&day).expect("day exists");
        list.remove(index);
        // A past day emptied out has no add row, so the dead card goes.
        if list.is_empty() && day < today() {
            self.days.remove(&day);
        }
    }

    pub fn complete_all(&mut self, day: NaiveDate) {
        if self.open_count(day) == 0 {
            return;
        }
        self.remember(true);
        for task in self.days.get_mut(&day).expect("day exists") {
            task.done = true;
        }
    }

    pub fn clear_all(&mut self, day: NaiveDate) {
        if self.days.get(&day).is_none_or(|list| list.is_empty()) {
            return;
        }
        self.remember(true);
        self.days.insert(day, Vec::new());
    }

    fn index_of(&self, day: NaiveDate, id: &str) -> Option<usize> {
        self.days.get(&day)?.iter().position(|t| t.id == id)
    }

    pub fn future_count(&self) -> usize {
        let today = today();
        self.days.keys().filter(|day| **day > today).count()
    }

    /// The next free upcoming day, tomorrow first; none once three are up.
    pub fn add_date(&mut self) -> Option<NaiveDate> {
        if self.future_count() >= FUTURE_GROUP_LIMIT {
            return None;
        }
        let mut day = today().succ_opt()?;
        while self.days.contains_key(&day) {
            day = day.succ_opt()?;
        }
        self.remember(false);
        self.days.insert(day, Vec::new());
        Some(day)
    }

    /// Done and total for today.
    pub fn today_counts(&self) -> (usize, usize) {
        let list = self.days.get(&today());
        (
            list.map_or(0, |l| l.iter().filter(|t| t.done).count()),
            list.map_or(0, Vec::len),
        )
    }

    /// The cards to show, newest first, with their rows filtered by the
    /// folded `needle`, and how many older days browsing left out.
    pub fn visible(&self, needle: &str) -> (Vec<(NaiveDate, Vec<Task>)>, usize) {
        let searching = !needle.is_empty();
        let mut hidden_older = 0;
        let mut cards = Vec::new();
        for (day, list) in self.days.iter().rev() {
            let diff = day_diff(*day);
            if !searching && diff < -BROWSE_PAST_DAYS {
                hidden_older += 1;
                continue;
            }
            let rows: Vec<Task> =
                if searching && !subsequence_match(needle, &date_search_text(*day, diff)) {
                    let rows: Vec<Task> = list
                        .iter()
                        .filter(|t| subsequence_match(needle, &t.name))
                        .cloned()
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    rows
                } else {
                    list.clone()
                };
            cards.push((*day, rows));
        }
        (cards, hidden_older)
    }

    // --- Analytics, the macOS TodoAnalytics math -------------------------------

    pub fn counts(&self) -> Counts {
        self.days
            .iter()
            .map(|(day, list)| (*day, (list.iter().filter(|t| t.done).count(), list.len())))
            .collect()
    }
}

/// Done and total over the current Sunday-based week or calendar month.
pub fn period_stat(counts: &Counts, period: Period) -> (usize, usize) {
    let today = today();
    let week_start = today - chrono::Days::new(today.weekday().num_days_from_sunday() as u64);
    let week_end = week_start + chrono::Days::new(6);
    counts
        .iter()
        .filter(|(day, _)| match period {
            Period::Week => **day >= week_start && **day <= week_end,
            Period::Month => day.year() == today.year() && day.month() == today.month(),
        })
        .fold((0, 0), |(d, t), (_, (done, total))| (d + done, t + total))
}

/// Consecutive days with a completed task, counting back from today. A
/// blank today does not break the streak, it just does not extend it.
pub fn streak_days(counts: &Counts) -> usize {
    let done_on = |day: NaiveDate| counts.get(&day).is_some_and(|(done, _)| *done > 0);
    let today = today();
    let mut streak = usize::from(done_on(today));
    let mut cursor = today;
    while let Some(previous) = cursor.pred_opt()
        && done_on(previous)
    {
        streak += 1;
        cursor = previous;
    }
    streak
}

/// Completed tasks per day over the trailing month, oldest first.
pub fn trend(counts: &Counts) -> Vec<usize> {
    let today = today();
    (0..TREND_DAYS)
        .rev()
        .map(|back| {
            let day = today - chrono::Days::new(back as u64);
            counts.get(&day).map_or(0, |(done, _)| *done)
        })
        .collect()
}

/// The heatmap's intensity step for a day's done count: 0, 1, 2 and 3 or
/// more land on steps 0, 1, 3 and 4 of a five-step ramp.
pub fn heat_level(done: usize) -> usize {
    match done {
        0 => 0,
        1 => 1,
        2 => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Todo {
        Todo {
            days: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            next_revision: 0,
            saved_revision: 0,
            loaded: true,
        }
    }

    #[test]
    fn phrases_cover_a_week_ahead() {
        assert_eq!(relative_phrase(0).as_deref(), Some("Today"));
        assert_eq!(relative_phrase(1).as_deref(), Some("Tomorrow"));
        assert_eq!(relative_phrase(-1).as_deref(), Some("Yesterday"));
        assert_eq!(relative_phrase(6).as_deref(), Some("In 6 days"));
        assert_eq!(relative_phrase(7), None);
        assert_eq!(relative_phrase(-2), None);
    }

    #[test]
    fn the_search_folds_case_marks_and_spaces() {
        assert!(subsequence_match(
            &fold_query("Jul 3"),
            "Tue Jul 3 Tomorrow"
        ));
        assert!(subsequence_match(&fold_query("di"), "đi chợ"));
        assert!(!subsequence_match(&fold_query("xyz"), "đi chợ"));
    }

    #[test]
    fn a_day_takes_three_open_tasks() {
        let mut todo = fresh();
        let day = today();
        assert!(todo.add_task(day, "a"));
        assert!(todo.add_task(day, "b"));
        assert!(todo.add_task(day, "c"));
        assert!(!todo.add_task(day, "d"));
        assert!(!todo.add_task(day, "   "));
        let id = todo.days()[&day][0].id.clone();
        todo.toggle(day, &id);
        assert!(todo.add_task(day, "d"));
        assert_eq!(todo.today_counts(), (1, 4));
    }

    #[test]
    fn undo_and_redo_walk_the_history_and_the_dirty_flag() {
        let mut todo = fresh();
        let day = today();
        todo.add_task(day, "a");
        assert!(todo.is_dirty());
        let (_, revision) = todo.for_save();
        todo.mark_saved(revision);
        assert!(!todo.is_dirty());
        todo.add_task(day, "b");
        assert!(todo.is_dirty());
        assert!(todo.undo());
        assert!(!todo.is_dirty());
        assert_eq!(todo.days()[&day].len(), 1);
        assert!(todo.redo());
        assert_eq!(todo.days()[&day].len(), 2);
        assert!(todo.is_dirty());
    }

    #[test]
    fn upcoming_days_stop_at_three() {
        let mut todo = fresh();
        assert!(todo.add_date().is_some());
        assert!(todo.add_date().is_some());
        assert!(todo.add_date().is_some());
        assert!(todo.add_date().is_none());
        assert_eq!(todo.future_count(), 3);
        // Placeholders do not light Save.
        assert!(!todo.is_dirty());
    }

    #[test]
    fn a_streak_counts_back_from_today() {
        let today = today();
        let mut counts = Counts::new();
        counts.insert(today - chrono::Days::new(1), (1, 2));
        counts.insert(today - chrono::Days::new(2), (2, 2));
        assert_eq!(streak_days(&counts), 2);
        counts.insert(today, (1, 1));
        assert_eq!(streak_days(&counts), 3);
        counts.insert(today - chrono::Days::new(1), (0, 2));
        assert_eq!(streak_days(&counts), 1);
    }
}
