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

/// `$HOME`, or the directory itself as a stand-in: a `~` left unresolved would
/// report every declared path as missing, which says nothing about the file.
fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
