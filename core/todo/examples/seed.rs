//! Seeds demo tasks into a todo database so the /todo UI (day cards, trend,
//! heatmap, streak) can be tested with a year of realistic history.
//!
//! Usage:
//!   cargo run -p look-todo --example seed -- [dev|main|<path>] [days-back] [tz-offset-hours]
//!
//! Targets:
//!   dev   (default) look.dev.db next to the app's real database; this is
//!         the file linows debug builds (cargo tauri dev) use automatically
//!   main  the app's real database (tasks are appended, never replaced)
//!   path  any explicit .db file
//!
//! Examples:
//!   cargo run -p look-todo --example seed
//!   cargo run -p look-todo --example seed -- dev 90 7
//!   cargo run -p look-todo --example seed -- /tmp/look-test.db 30
//!
//! Day keys are derived from UTC; pass your UTC offset (e.g. 7 for UTC+7) so
//! "today" lines up with your local date around midnight.

use look_todo::{TodoStore, TodoTask};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_DAYS: i64 = 365;
const SECS_PER_DAY: i64 = 86_400;

const SAMPLE_NAMES: &[&str] = &[
    "Review PR",
    "Write weekly notes",
    "Fix flaky test",
    "Read one paper",
    "Inbox zero",
    "Ship release",
    "Update docs",
    "Plan tomorrow",
    "Workout",
    "Call home",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let target = args.next().unwrap_or_else(|| "dev".to_string());
    let days: i64 = args
        .next()
        .map(|v| v.parse().expect("days-back must be a number"))
        .unwrap_or(DEFAULT_DAYS);
    let tz_offset_hours: i64 = args
        .next()
        .map(|v| v.parse().expect("tz-offset-hours must be a number"))
        .unwrap_or(0);

    let path = resolve_db_path(&target);
    let mut store = TodoStore::open(&path).expect("open db");
    let mut tasks = store.list().expect("list");
    let existing = tasks.len();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let today = (now + tz_offset_hours * 3_600) / SECS_PER_DAY;
    let mut rng = now as u64;

    for back in 0..=days {
        // Roughly a quarter of days stay empty; the rest get 1-3 tasks.
        let count = match next(&mut rng) % 8 {
            0 | 1 => 0,
            2..=4 => 1,
            5 | 6 => 2,
            _ => 3,
        };
        let due_date = civil_date(today - back);
        for i in 0..count {
            // Past tasks are mostly done (a few stay open to show OVERDUE);
            // today's alternate so the quick view has unfinished entries.
            let done = if back == 0 {
                i % 2 == 0
            } else {
                next(&mut rng) % 10 < 8
            };
            tasks.push(TodoTask {
                id: format!("seed{:06x}", next(&mut rng) & 0xff_ffff),
                name: SAMPLE_NAMES[next(&mut rng) as usize % SAMPLE_NAMES.len()].to_string(),
                done,
                due_date: due_date.clone(),
                created_at_unix_s: now - back * SECS_PER_DAY,
            });
        }
    }

    store.save(&tasks).expect("save");
    println!(
        "{}: {existing} existing + {} seeded = {} tasks ({days} days back)",
        path.display(),
        tasks.len() - existing,
        tasks.len(),
    );
    if target == "dev" {
        println!("debug builds (cargo tauri dev) read this file automatically");
    }
}

/// `dev` -> look.dev.db beside the app database (the same file linows
/// main.rs points debug builds at), `main` -> the app database itself,
/// anything else -> literal path. App-database resolution mirrors linows
/// state.rs / bridge ffi state.rs (LOOK_DB_PATH, then the platform default);
/// kept in sync by hand since the app crates can't be dependencies of core.
fn resolve_db_path(target: &str) -> PathBuf {
    match target {
        "main" => app_db_path(),
        "dev" => {
            let mut path = app_db_path();
            path.set_file_name("look.dev.db");
            path
        }
        explicit => PathBuf::from(explicit),
    }
}

fn app_db_path() -> PathBuf {
    if let Ok(custom) = std::env::var("LOOK_DB_PATH")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom.trim());
    }

    let home = || std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    #[cfg(target_os = "windows")]
    if let Ok(base) = std::env::var("LOCALAPPDATA")
        && !base.trim().is_empty()
    {
        return PathBuf::from(base.trim()).join("look").join("look.db");
    }

    #[cfg(target_os = "macos")]
    return PathBuf::from(home())
        .join("Library")
        .join("Application Support")
        .join("look")
        .join("look.db");

    #[cfg(not(target_os = "macos"))]
    {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME")
            && !data_home.trim().is_empty()
        {
            return PathBuf::from(data_home.trim()).join("look").join("look.db");
        }
        PathBuf::from(home())
            .join(".local")
            .join("share")
            .join("look")
            .join("look.db")
    }
}

// Tiny xorshift so the example needs no rand dependency.
fn next(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Days-since-epoch to "yyyy-MM-dd" (Howard Hinnant's civil-from-days),
/// so the example needs no date dependency either.
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
