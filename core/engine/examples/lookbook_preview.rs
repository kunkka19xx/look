//! Reads a lookbook checkout the way the browser will, and says what each
//! entry would do on this machine.
//!
//!     cargo run -p look-engine --example lookbook_preview -- ../lookbook
//!     cargo run -p look-engine --example lookbook_preview -- ../lookbook docker
//!
//! The browser fetches `index.json` and the files it names over HTTP; this
//! reads the same two things off disk instead. Everything after that is the
//! code the shell will call, so what prints here is what a card will say.
//!
//! An entry for another platform is listed as such rather than checked. Its
//! commands are not missing, they are simply someone else's.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct Catalog {
    sources: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    name: String,
    platforms: Vec<String>,
    requires: Vec<String>,
    files: Vec<String>,
    path: String,
}

const THIS_PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "linux"
};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().expect("path to a lookbook checkout"));
    let only = args.next();

    let index = root.join("index.json");
    let catalog: Catalog = match std::fs::read_to_string(&index) {
        Ok(text) => serde_json::from_str(&text).expect("index.json parses"),
        Err(err) => {
            eprintln!("cannot read {}: {err}", index.display());
            eprintln!("run `make index` in the checkout first");
            std::process::exit(2);
        }
    };

    let home = home();
    for entry in &catalog.sources {
        if only.as_deref().is_some_and(|id| id != entry.id) {
            continue;
        }
        println!("{} ({})", entry.name, entry.id);

        if !entry.platforms.iter().any(|name| name == THIS_PLATFORM) {
            println!("  not for {THIS_PLATFORM}: {}", entry.platforms.join(", "));
            continue;
        }

        for program in look_engine::lookbook::missing_programs(&entry.requires) {
            println!("  requires: {program}, which is not installed");
        }

        for file in &entry.files {
            let path = root.join(&entry.path).join(file);
            let Ok(contents) = std::fs::read_to_string(&path) else {
                println!("  cannot read {}", path.display());
                continue;
            };
            match look_engine::lookbook::preview(&contents, &home) {
                Ok(preview) => say(file, &preview),
                Err(err) => println!("  problem: {file}: {err}"),
            }
        }
    }
}

fn say(file: &str, preview: &look_engine::lookbook::Preview) {
    for problem in &preview.problems {
        println!("  problem: {file}: {problem}");
    }
    for block in &preview.blocks {
        println!("  {} [{}] {}", file, block.id, block.kind);
        for need in &block.needs {
            println!("    needs: {}", need.message);
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new("/").to_path_buf())
}
