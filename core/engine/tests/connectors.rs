//! Connector lifecycle, against the reference connector in `src/refcon.rs`.
//!
//! An integration test rather than a unit one so it can spawn that binary:
//! `CARGO_BIN_EXE_*` is only set out here. These used to be unit tests with a
//! `#!/bin/sh` fixture, which is why they never ran on Windows.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The directories and the row cache are named by environment variables, which
/// are per process, so these run one at a time.
fn alone() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

struct Fixture {
    base: PathBuf,
    cache: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let base = std::env::temp_dir().join(format!("look-refcon-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("sources");
        let cache = base.join("cache");
        let connectors = base.join("connectors");
        for path in [&dir, &cache, &connectors] {
            std::fs::create_dir_all(path).expect("fixture dir");
        }
        unsafe { std::env::set_var(look_sources::SOURCES_DIR_ENV, &dir) };
        unsafe { std::env::set_var(look_sources::CONNECTORS_DIR_ENV, &connectors) };
        unsafe { std::env::set_var(look_engine::index::ROWS_CACHE_DIR_ENV, &cache) };
        Self { base, cache }
    }

    /// The same, plus a control the rows carry.
    fn install_with_control(&self) {
        self.write(
            "serve",
            "control = { id = \"lamp\", kind = \"toggle\", title = \"Lamp\", on = \"On\", off = \"Off\" }\n",
        );
    }

    /// The same, plus an `archive` verb the action menu would offer.
    fn install_with_verb(&self, mode: &str) {
        self.write(
            mode,
            "then = [\"archive\"]\n\n[archive]\nname = \"Archive\"\nask = \"archive\"\n",
        );
    }

    /// Installs a manifest whose block runs the reference connector in `mode`.
    ///
    /// The path is written as a TOML literal string: a Windows path is full of
    /// backslashes, which a basic string would read as escapes. Inside it the
    /// path is quoted for the shell, since a temp directory may hold a space.
    fn install(&self, mode: &str) {
        self.write(mode, "");
    }

    fn write(&self, mode: &str, extra: &str) {
        let exe = Path::new(env!("CARGO_BIN_EXE_refcon"))
            .display()
            .to_string();
        assert!(
            !exe.contains('\''),
            "the path would break the literal string"
        );
        std::fs::write(
            self.base.join("connectors").join("myapp.toml"),
            format!("[notes]\nconnect = '\"{exe}\" myapp.notes {mode}'\n{extra}"),
        )
        .expect("manifest");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        look_engine::connectors::stop();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Counts the blocks Look was told about, which is how a push reaches a shell.
fn counter() -> (Arc<AtomicUsize>, look_engine::connectors::OnRows) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    (
        count,
        Arc::new(move |_block: &str| {
            seen.fetch_add(1, Ordering::Release);
        }),
    )
}

/// Nothing blocks on a connector, so a test waits the way the app does: it
/// carries on and looks again.
fn until(count: &AtomicUsize, want: usize, patience: Duration) {
    let deadline = Instant::now() + patience;
    while count.load(Ordering::Acquire) < want && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn a_connector_pushes_rows_into_the_cache() {
    let _alone = alone();
    let fixture = Fixture::new("push");
    fixture.install("serve");

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    until(&count, 1, Duration::from_secs(10));
    look_engine::connectors::stop();

    assert_eq!(count.load(Ordering::Acquire), 1);
    let text = std::fs::read_to_string(fixture.cache.join("myapp.notes")).expect("rows cached");
    assert!(text.contains("\"Alpha\""), "{text}");
}

/// "Restarted on next use": a connector that exited comes back when the
/// launcher next opens, without re-reading any directory.
#[test]
fn a_dead_connector_is_restarted() {
    let _alone = alone();
    let fixture = Fixture::new("restart");
    fixture.install("once");

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows.clone());
    until(&count, 1, Duration::from_secs(10));

    // The frame is handled before the child has necessarily exited, and
    // `restart_dead` restarts nothing while it is still alive. Asking once is
    // a race; asking until it answers is the behaviour being tested.
    let deadline = Instant::now() + Duration::from_secs(20);
    while count.load(Ordering::Acquire) < 2 && Instant::now() < deadline {
        look_engine::connectors::restart_dead(on_rows.clone());
        std::thread::sleep(Duration::from_millis(50));
    }
    look_engine::connectors::stop();

    assert_eq!(count.load(Ordering::Acquire), 2);
}

/// A connector watching something unchanged pushes the same rows over and
/// over, and that should cost nothing: no write, and nothing re-read.
#[test]
fn the_same_rows_again_are_not_news() {
    let _alone = alone();
    let fixture = Fixture::new("dedupe");
    fixture.install("same");

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    until(&count, 1, Duration::from_secs(10));
    // Past the second send, which the connector spaces out on purpose.
    std::thread::sleep(Duration::from_millis(700));
    look_engine::connectors::stop();

    assert_eq!(count.load(Ordering::Acquire), 1);
}

/// A verb is planned before it runs, and the question names the rows: the
/// whole point, since a declared `confirm` string cannot.
#[test]
fn a_verb_is_planned_then_run() {
    let _alone = alone();
    let fixture = Fixture::new("verb");
    fixture.install_with_verb("serve");

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    until(&count, 1, Duration::from_secs(10));

    let candidate =
        look_indexing::CandidateIdKind::source_row_candidate_id("myapp.notes", &[], "n1");
    let row = look_engine::sources::row_context(&candidate, "Alpha", "", "", Vec::new());
    // What the connector calls it, not what Look calls it.
    assert_eq!(row.id, "n1");

    let planned = look_engine::sources::plan_verb("myapp.archive", &candidate, &row);
    assert_eq!(planned.error, None);
    assert_eq!(planned.title, "archive 1 row(s)");
    assert_eq!(planned.detail.as_deref(), Some("n1"));

    let ran = look_engine::sources::run_verb(&candidate, planned.plan);
    assert_eq!(ran.error, None);
    assert!(ran.ok);
    assert!(ran.summary.is_some());

    look_engine::connectors::stop();
}

/// A press answers with where the control ended up, so a shell never assumes
/// the connector agreed.
#[test]
fn a_control_is_read_and_pressed() {
    let _alone = alone();
    let fixture = Fixture::new("control");
    fixture.install_with_control();

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    until(&count, 1, Duration::from_secs(10));

    let action = look_engine::qactions::control_action_id("myapp.notes", "lamp");
    assert_eq!(look_engine::sources::control(&action, false).error, None);
    assert!(!look_engine::sources::control(&action, false).on);
    assert!(
        look_engine::sources::control(&action, true).on,
        "press turns it on"
    );
    assert!(
        look_engine::sources::control(&action, false).on,
        "and it stays"
    );

    look_engine::connectors::stop();
}

/// The same control, placed on the strip. A tile and a row control are one
/// declaration: the drawing only says where it goes.
#[test]
fn a_tile_reads_and_presses_the_same_control() {
    let _alone = alone();
    let fixture = Fixture::new("tile");
    fixture.install_with_control();

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    until(&count, 1, Duration::from_secs(10));

    let def = look_engine::launchpad::TileDef {
        control: Some("myapp.notes#lamp".into()),
        ..Default::default()
    };
    // Off, then pressed on, and the tile reads as a toggle rather than as a
    // readout that happens to say "On".
    let value = look_engine::launchpad_values::value_for_test("lamp", &def, false).expect("read");
    assert_eq!(value.state.as_deref(), Some("off"));
    let value = look_engine::launchpad_values::value_for_test("lamp", &def, true).expect("press");
    assert_eq!(value.state.as_deref(), Some("on"));
    assert_eq!(value.value, "On");

    look_engine::connectors::stop();
}

/// Nothing to confirm is not the same as a plan that says it will do nothing.
#[test]
fn a_verb_with_no_connector_running_says_so() {
    let _alone = alone();
    let fixture = Fixture::new("verbdead");
    fixture.install_with_verb("serve");

    let candidate =
        look_indexing::CandidateIdKind::source_row_candidate_id("myapp.notes", &[], "n1");
    let row = look_engine::sources::row_context(&candidate, "Alpha", "", "", Vec::new());
    let planned = look_engine::sources::plan_verb("myapp.archive", &candidate, &row);
    assert_eq!(planned.error.as_deref(), Some("connector is not running"));
}

/// The cap counts the frame, not the newline after it, so a frame of exactly
/// `MAX_FRAME_BYTES` is one Look reads. Budgeting the delimiter inside the cap
/// rejected it, which the limit in `docs/integrating.md` does not.
#[test]
fn a_frame_at_the_cap_is_read() {
    let _alone = alone();
    let fixture = Fixture::new("atcap");
    fixture.install("atcap");

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    until(&count, 1, Duration::from_secs(10));
    look_engine::connectors::stop();

    assert_eq!(count.load(Ordering::Acquire), 1);
}

/// And one byte past it is not. `lines()` would read a frame of any length
/// into memory, so a connector could hand Look a gigabyte.
#[test]
fn an_oversized_frame_ends_the_connector_rather_than_look() {
    let _alone = alone();
    let fixture = Fixture::new("oversized");
    fixture.install("oversized");

    let (count, on_rows) = counter();
    look_engine::connectors::start(&fixture.base, on_rows);
    std::thread::sleep(Duration::from_secs(2));
    look_engine::connectors::stop();

    // Nothing stored, and Look is still here to assert it.
    assert_eq!(count.load(Ordering::Acquire), 0);
}
