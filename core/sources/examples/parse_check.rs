//! Reads a sources directory the way Look does and reports what it made of it.
//!
//!     cargo run -p look-sources --example parse_check -- ~/.look/sources
//!
//! `problem:` is a file Look could not use. `needs:` is a file it loaded and
//! cannot run here: a copied example still pointing at its author's folders, or
//! naming a program this machine does not have. That second list is why a
//! freshly pulled example does nothing, so it is checked against `$HOME` and
//! this machine's `PATH` rather than in the abstract.

use std::path::{Path, PathBuf};

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let loaded = look_sources::load_dir(Path::new(&dir));
    for problem in &loaded.problems {
        println!("problem: {}", problem.message);
    }
    check_setup(Path::new(&dir));
    let home = home();
    for block in &loaded.blocks {
        println!(
            "block {} name={:?} icon={:?} enabled={} preview={:?} unknown={:?}",
            block.id, block.name, block.icon, block.enabled, block.preview, block.unknown_keys
        );
        for unready in look_sources::check(block, &home) {
            println!("needs: [{}] {}", block.id, unready.message());
        }
    }
}

/// The `[[setup]]` fields each declaration asks for, checked against the file
/// they point into.
///
/// A field naming a key that is not there, or a `match` that is not unique,
/// fails at install time in front of a user who did nothing wrong. Reported
/// as `problem:` so it fails the author's own check run instead.
fn check_setup(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        // Catalog data for lookbook's `make index`, not a declaration.
        .filter(|path| path.file_name().is_some_and(|name| name != "meta.toml"))
        .collect();
    files.sort();

    for path in files {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        // Unparseable TOML is already the loudest thing `load_dir` says, so
        // nothing is added here for it.
        if let Ok((fields, problems)) = look_sources::parse_setup(&contents) {
            for message in problems {
                println!("problem: {name}: {message}");
            }
            for message in look_sources::validate(&contents, &fields) {
                println!("problem: {name}: {message}");
            }
        }
    }
}

/// `$HOME`, or the directory itself as a stand-in: a `~` left unresolved would
/// report every declared path as missing, which says nothing about the file.
fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
