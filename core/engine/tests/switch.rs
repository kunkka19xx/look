//! Turning a declaration off from the Extend screen.
//!
//! An integration test rather than a unit one because the directories are
//! named by environment variables, which are per process: a unit test that set
//! them would be setting them for every other test in the crate.

use std::path::Path;

/// One test, deliberately. The environment is process-wide, so a second test
/// in this binary would be racing this one for it.
#[test]
fn turns_a_declaration_off_and_back_on() {
    let base = std::env::temp_dir().join(format!("look-switch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let sources = base.join("sources");
    std::fs::create_dir_all(&sources).expect("fixture dir");
    unsafe { std::env::set_var(look_sources::SOURCES_DIR_ENV, &sources) };
    unsafe { std::env::set_var(look_sources::CONNECTORS_DIR_ENV, base.join("connectors")) };

    let file = sources.join("mine.toml");
    std::fs::write(&file, "# mine\n[notes]\ndir = \"/tmp\"\n").expect("fixture file");

    let off = look_engine::sources::set_enabled(&file, false, &base);
    assert!(off.problems.is_empty(), "{:?}", off.problems);
    assert!(off.changed);
    let written = std::fs::read_to_string(&file).expect("written");
    assert!(written.starts_with("# mine\n"), "{written}");
    assert!(!declared(&base).iter().any(|block| block.enabled));

    // Already off: nothing to write, and nothing failed either.
    let again = look_engine::sources::set_enabled(&file, false, &base);
    assert!(!again.changed);
    assert!(again.problems.is_empty());

    let on = look_engine::sources::set_enabled(&file, true, &base);
    assert!(on.changed, "{:?}", on.problems);
    assert!(declared(&base).iter().all(|block| block.enabled));

    // A path the screen never showed. The guard is here because nothing below
    // this point asks again whether the file is a declaration at all.
    let outside = base.join("elsewhere.toml");
    std::fs::write(&outside, "[notes]\ndir = \"/tmp\"\n").expect("fixture file");
    let refused = look_engine::sources::set_enabled(&outside, false, &base);
    assert!(!refused.changed);
    assert_eq!(refused.problems.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&outside).expect("untouched"),
        "[notes]\ndir = \"/tmp\"\n"
    );

    let _ = std::fs::remove_dir_all(&base);
}

fn declared(home: &Path) -> Vec<look_sources::Block> {
    look_sources::load_dir(&look_sources::sources_dir(home)).blocks
}
