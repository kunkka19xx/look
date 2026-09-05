//! What the tiles a user declared currently show.
//!
//! The launchpad renders before the user has typed anything, so a slow command
//! here is felt on every launch. Hence the split: [`cached`] never spawns and is
//! what the shells call while drawing; [`refresh`] does the work off that path.

use crate::launchpad::{TileDef, TileValue};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// Overrides the cache directory, for tests and for a second configuration.
pub const CACHE_DIR_ENV: &str = "LOOK_TILES_CACHE_DIR";

const CACHE_DIR_NAME: &str = ".look/cache/tiles";

/// Under `preview`'s five: a preview runs while the user is already waiting on a
/// row, a tile while the window is trying to appear.
const TIMEOUT: Duration = Duration::from_secs(2);

/// A tile prints a line or two; past this the command has misunderstood the job.
const MAX_BYTES: usize = 16 * 1024;

/// Five placed tiles must not fork five shells on every open. The rest wait for
/// the next one, rendering from cache meanwhile.
const MAX_CONCURRENT: usize = 3;

/// Staleness when the tile does not say: short enough for a countdown, long
/// enough that most opens spawn nothing.
const DEFAULT_REFRESH: Duration = Duration::from_secs(60);

/// How a refresh went. `errors` are already worded for display.
#[derive(Debug, Clone, Default)]
pub struct RefreshOutcome {
    pub refreshed: usize,
    pub errors: Vec<String>,
}

fn cache_dir() -> Option<PathBuf> {
    crate::index::cache_dir_named(CACHE_DIR_ENV, CACHE_DIR_NAME)
}

/// Whether a tile's name can stand as a filename in the cache. The name comes
/// from the drawing, so `..` or a slash in it would write outside the cache.
/// Crate-public because the drawing is where a bad one can still be reported.
pub(crate) fn is_cacheable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// One file per tile, named for the tile.
fn cache_path(name: &str) -> Option<PathBuf> {
    is_cacheable_name(name)
        .then(cache_dir)?
        .map(|dir| dir.join(name))
}

/// Tiles whose command is running right now.
fn in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Every tile value on disk. Spawns nothing, so it is safe while drawing. No
/// entry means never run or printed nothing - both draw the placeholder.
pub fn cached() -> HashMap<String, TileValue> {
    let mut values = HashMap::new();
    let Some(dir) = cache_dir() else {
        return values;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return values;
    };

    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str::<TileValue>(&text) {
            values.insert(name, value);
        }
    }
    values
}

/// `refresh` is a maximum staleness, not an interval: an open inside the window
/// spawns nothing, which is what makes a per-open tile affordable.
fn is_stale(name: &str, def: &TileDef) -> bool {
    // Never: there is no command, so there is nothing that could go stale.
    if def.value.is_none() {
        return false;
    }
    let Some(path) = cache_path(name) else {
        return false;
    };
    let Ok(written) = std::fs::metadata(&path).and_then(|meta| meta.modified()) else {
        return true; // never run
    };
    let age = SystemTime::now()
        .duration_since(written)
        .unwrap_or(Duration::ZERO);
    age >= def.refresh.unwrap_or(DEFAULT_REFRESH)
}

/// Runs one tile's command and stores what it printed.
fn run_one(name: &str, def: &TileDef) -> Result<(), String> {
    // A tile that only acts has nothing to run until it is pressed.
    let Some(command) = def.value.as_deref() else {
        return Ok(());
    };
    let output = look_sources::capture(command, None, TIMEOUT, MAX_BYTES)
        .map_err(|err| format!("tile \"{name}\": {err}"))?;

    let Some(path) = cache_path(name) else {
        return Err(format!("tile \"{name}\": not a name that can be cached"));
    };

    // Printing nothing hides the tile. Inverts the `run`-block rule, where empty
    // means "keep the rows you had": a stale meeting is worse than no meeting.
    if output.trim().is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }

    // Parsed here, not in the shells: reported once, and the last good value
    // survives rather than a shell rendering rubbish.
    serde_json::from_str::<TileValue>(output.trim())
        .map_err(|err| format!("tile \"{name}\": did not print a tile value ({err})"))?;

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, output.trim())
        .map_err(|err| format!("tile \"{name}\": could not be cached ({err})"))
}

/// Runs every stale tile and stores what it prints. Blocking and spawning: call
/// off the drawing thread. A failure is per-tile - it keeps its last value.
pub fn refresh() -> RefreshOutcome {
    let defs = crate::launchpad::layout().defs;
    // Here because the placed names are already in hand: without it a renamed
    // tile's last reading is read and parsed by every `cached()` call forever.
    sweep(&defs.keys().cloned().collect());
    refresh_defs(&defs)
}

/// The refresh, against a given set of tiles, so it can be tested without a
/// super-actions.toml.
pub fn refresh_defs(defs: &HashMap<String, TileDef>) -> RefreshOutcome {
    let mut outcome = RefreshOutcome::default();

    // Claimed before any spawn, so five opens in ten seconds run one copy. A
    // tile already running is skipped: that run gives the same answer.
    let claimed: Vec<(&String, &TileDef)> = {
        let mut flight = in_flight().lock().unwrap_or_else(|err| err.into_inner());
        defs.iter()
            .filter(|(name, def)| is_stale(name, def) && flight.insert((*name).clone()))
            .collect()
    };

    for chunk in claimed.chunks(MAX_CONCURRENT) {
        let results: Vec<Result<(), String>> = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(name, def)| scope.spawn(move || run_one(name, def)))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_else(|_| Err("panicked".into())))
                .collect()
        });

        for result in results {
            match result {
                Ok(()) => outcome.refreshed += 1,
                Err(message) => outcome.errors.push(message),
            }
        }
    }

    if !claimed.is_empty() {
        let mut flight = in_flight().lock().unwrap_or_else(|err| err.into_inner());
        for (name, _) in &claimed {
            flight.remove(*name);
        }
    }

    outcome
}

/// Runs a user tile's `press`, detached.
///
/// The shell names the tile, never the command, so it cannot be talked into
/// running something the drawing did not declare.
pub fn press(name: &str) -> Result<(), String> {
    let Some(def) = crate::launchpad::layout().defs.remove(name) else {
        return Err(format!("\"{name}\" is not a tile that does anything"));
    };
    press_def(name, &def)
}

/// The press, against a given tile, so it can be tested without a
/// super-actions.toml. Split the way `refresh_defs` is split from `refresh`.
fn press_def(name: &str, def: &TileDef) -> Result<(), String> {
    let Some(command) = def.press.clone() else {
        return Err(format!("\"{name}\" has no press command"));
    };
    // No row context: a tile is not a row, so there are no placeholders to
    // expand and the command runs exactly as it was written.
    look_sources::perform(&[command], None)
        .into_iter()
        .find_map(|step| step.error)
        .map_or(Ok(()), Err)?;

    // Read again rather than wait out the refresh window: a press changes what
    // the tile reports. Through `run_one`, so a failed read keeps the last good
    // value. A press is detached, so a command still settling lands next time.
    let _ = run_one(name, def);
    Ok(())
}

/// Drops values for tiles the drawing no longer names.
pub fn sweep(keep: &HashSet<String>) -> usize {
    let Some(dir) = cache_dir() else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !keep.contains(&name) && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialised: these share one cache directory and one in-flight set.
    static GUARD: Mutex<()> = Mutex::new(());

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("look-tiles-{}-{label}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("temp dir");
            unsafe { std::env::set_var(CACHE_DIR_ENV, &dir) };
            Self { dir }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(CACHE_DIR_ENV) };
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Always due. These tests refresh twice to watch a value change, and the
    /// default window is a minute, so the second call would otherwise no-op.
    fn def(value: &str) -> TileDef {
        TileDef {
            value: Some(value.to_string()),
            refresh: Some(Duration::ZERO),
            ..Default::default()
        }
    }

    fn defs(pairs: &[(&str, &str)]) -> HashMap<String, TileDef> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), def(value)))
            .collect()
    }

    #[test]
    fn a_tile_shows_what_its_command_printed() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("prints");

        let outcome = refresh_defs(&defs(&[(
            "ci",
            r#"printf '{"value":"3 failing","caption":"CI"}'"#,
        )]));
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(outcome.refreshed, 1);

        let values = cached();
        assert_eq!(values["ci"].value, "3 failing");
        assert_eq!(values["ci"].caption.as_deref(), Some("CI"));
    }

    #[test]
    fn printing_nothing_hides_the_tile() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("empty");

        refresh_defs(&defs(&[("meeting", r#"printf '{"value":"in 24m"}'"#)]));
        assert!(cached().contains_key("meeting"), "it had a meeting");

        // The day's meetings are over, so the command prints nothing and the
        // tile goes away. This is the INVERSE of the run-block rule, where
        // empty output means "keep the rows you had".
        refresh_defs(&defs(&[("meeting", "true")]));
        assert!(
            !cached().contains_key("meeting"),
            "an empty answer removes the tile rather than keeping the old one"
        );
    }

    /// A command that writes to stderr and exits non-zero, in each shell's own
    /// language. `cmd` reads `;` as an argument rather than a separator, so the
    /// POSIX spelling exits zero there with nothing on stdout - which is the
    /// signal for "printed nothing", the opposite of what this test is about.
    #[cfg(not(windows))]
    const FAILING_COMMAND: &str = "echo boom >&2; exit 1";
    #[cfg(windows)]
    const FAILING_COMMAND: &str = "echo boom 1>&2& exit 1";

    #[test]
    fn a_failing_command_keeps_the_last_good_value_and_says_why() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("fails");

        // Seeded by writing the value rather than by running a command that
        // prints it: that a good run is cached is
        // `a_tile_shows_what_its_command_printed`'s subject, and a second shell
        // here is one more spawn that can miss the two-second cap on a loaded
        // machine - which lands as "no entry found for key" three lines below,
        // blaming the assertion rather than the seed.
        std::fs::write(
            cache_path("ci").expect("a cache path"),
            r#"{"value":"green"}"#,
        )
        .expect("a seeded value");

        let outcome = refresh_defs(&defs(&[("ci", FAILING_COMMAND)]));

        assert_eq!(
            cached()["ci"].value,
            "green",
            "a broken run must not blank a tile that was working"
        );
        assert_eq!(outcome.errors.len(), 1);
        assert!(outcome.errors[0].contains("ci"), "{:?}", outcome.errors);
    }

    #[test]
    fn a_press_reads_the_tile_again_rather_than_waiting_out_its_window() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("pressed");

        // An hour, so a refresh pass on its own would run nothing at all.
        let readout = |value: &str| TileDef {
            value: Some(value.to_string()),
            refresh: Some(Duration::from_secs(3600)),
            press: Some("true".to_string()),
            ..Default::default()
        };
        let one = |def: TileDef| -> HashMap<String, TileDef> {
            [("ci".to_string(), def)].into_iter().collect()
        };

        refresh_defs(&one(readout(r#"printf '{"value":"before"}'"#)));
        assert_eq!(cached()["ci"].value, "before");

        let changed = readout(r#"printf '{"value":"after"}'"#);
        assert_eq!(
            refresh_defs(&one(changed.clone())).refreshed,
            0,
            "inside the window there is nothing for a refresh to do"
        );
        assert_eq!(cached()["ci"].value, "before");

        // A press changes what the tile reports, so it reads its own tile back
        // rather than showing the old value until the window runs out.
        press_def("ci", &changed).expect("the press ran");
        assert_eq!(cached()["ci"].value, "after");
    }

    #[test]
    fn output_that_is_not_a_tile_value_is_refused_rather_than_stored() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("garbage");

        let outcome = refresh_defs(&defs(&[("ci", "echo hello")]));
        assert!(cached().get("ci").is_none());
        assert!(
            outcome.errors[0].contains("did not print a tile value"),
            "{:?}",
            outcome.errors
        );
    }

    #[test]
    fn a_value_inside_its_refresh_window_spawns_nothing() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("fresh");

        let mut tiles = defs(&[("ci", r#"printf '{"value":"first"}'"#)]);
        tiles.get_mut("ci").unwrap().refresh = Some(Duration::from_secs(3600));
        assert_eq!(refresh_defs(&tiles).refreshed, 1);

        // `refresh` is a maximum staleness, not an interval: the second open is
        // inside the window, so nothing runs at all. That is what makes a tile
        // on every launcher open affordable.
        tiles.get_mut("ci").unwrap().value = Some(r#"printf '{"value":"second"}'"#.to_string());
        assert_eq!(refresh_defs(&tiles).refreshed, 0, "no spawn inside the TTL");
        assert_eq!(cached()["ci"].value, "first");
    }

    #[test]
    fn a_hung_command_cannot_hold_the_launchpad_open() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("hangs");

        let started = std::time::Instant::now();
        let outcome = refresh_defs(&defs(&[("slow", "sleep 30")]));
        let elapsed = started.elapsed();

        assert!(
            elapsed < TIMEOUT + Duration::from_secs(2),
            "took {elapsed:?}, so the deadline did not hold"
        );
        assert!(
            outcome.errors[0].contains("timed out"),
            "{:?}",
            outcome.errors
        );
    }

    #[test]
    fn a_name_that_would_escape_the_cache_directory_is_refused() {
        // The name comes from the user's drawing, and it is a filename here.
        assert!(cache_path("../../etc/passwd").is_none());
        assert!(cache_path("nested/name").is_none());
        assert!(cache_path("").is_none());
        assert!(cache_path("ci-status_2").is_some());
    }

    #[test]
    fn tiles_the_drawing_no_longer_names_stop_being_cached() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("sweep");

        refresh_defs(&defs(&[
            ("ci", r#"printf '{"value":"a"}'"#),
            ("old", r#"printf '{"value":"b"}'"#),
        ]));
        assert_eq!(cached().len(), 2);

        let keep: HashSet<String> = ["ci".to_string()].into_iter().collect();
        assert_eq!(sweep(&keep), 1);
        assert!(cached().contains_key("ci"));
        assert!(!cached().contains_key("old"));
    }
}
