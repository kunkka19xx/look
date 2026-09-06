//! Reads a `super-actions.toml` the way Look does and reports what it made of
//! it. The sibling of `look-sources`' `parse_check`, and there for the same
//! reason: lookbook validates its tile examples with Look's own resolver rather
//! than a second one that would only ever teach us what it thinks.
//!
//!     cargo run -p look-engine --example launchpad_check -- path/to/super-actions.toml
//!
//! `resolve` never fails - a drawing it cannot trust falls back to the built-in
//! grid so the strip is never empty - so every complaint arrives as a warning.
//! They are printed `problem:` to match `parse_check`, which is what a caller
//! greps for.

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("path to a super-actions.toml");
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("cannot read {path}: {err}");
            std::process::exit(2);
        }
    };

    let resolved = look_engine::launchpad::resolve(&contents);

    for warning in &resolved.warnings {
        println!("problem: {warning}");
    }

    println!("grid {}x{}", resolved.columns, resolved.rows);

    for tile in &resolved.tiles {
        let kind = if resolved.defs.contains_key(&tile.action_id) {
            "user"
        } else {
            "builtin"
        };
        println!(
            "tile {} kind={kind} at={},{} span={}x{} title={:?} mnemonic={:?} pressable={} \
             has_value={}",
            tile.action_id,
            tile.col,
            tile.row,
            tile.col_span,
            tile.row_span,
            tile.title,
            tile.mnemonic,
            tile.pressable,
            tile.has_value,
        );
    }

    // A tile whose name never made it into the drawing is declared and unused,
    // which is the one mistake the warnings above stay quiet about: the file is
    // valid, the tile simply is not on screen. `resolved.defs` cannot answer
    // this - it holds only what was placed - so the declarations are read back
    // from the same TOML rather than inferred.
    for name in declared_tiles(&contents) {
        if !resolved.tiles.iter().any(|tile| tile.action_id == name) {
            println!("problem: [tiles.{name}] is declared but never drawn in `layout`");
        }
    }
}

/// The `[tiles.<name>]` entries the file declares, in the order written.
///
/// Unparseable TOML is already the loudest warning `resolve` produces, so this
/// stays quiet and yields nothing rather than complaining a second time.
fn declared_tiles(contents: &str) -> Vec<String> {
    let Ok(root) = toml::from_str::<toml::Value>(contents) else {
        return Vec::new();
    };
    let Some(tiles) = root.get("tiles").and_then(|tiles| tiles.as_table()) else {
        return Vec::new();
    };
    tiles.keys().cloned().collect()
}
