//! The user's launchpad layout, `~/.look/launchpad.toml`.
//!
//! Here rather than in `look_qactions` for one reason: that crate is the
//! platform-neutral action catalog and has no way to find a home directory.
//! Giving it one would mean a second derivation of `$HOME` / `$USERPROFILE`,
//! and `config_path` already records why that is a trap - the two can name
//! different directories on Windows, so callers ordering them differently
//! resolve different files.
//!
//! So the split is: `look_qactions::launchpad_layout()` owns what the DEFAULT
//! grid is, and this module owns what the user asked for.

use look_qactions::LaunchpadTile;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Overrides the file, for tests and for anyone running two configurations.
pub const LAUNCHPAD_FILE_ENV: &str = "LOOK_LAUNCHPAD_FILE";

/// The launchpad is the empty state of a window that has to appear instantly,
/// so its height is laid out against a known maximum rather than an open-ended
/// one. Three is today's layout; five leaves room for a band of your own tiles.
pub const MAX_ROWS: usize = 5;

/// A gap the user drew on purpose.
const HOLE: &str = ".";

/// The grid the shells draw, and anything wrong with the file that produced it.
///
/// `tiles` is never empty: every failure path falls back to the built-in
/// default, because a launchpad that renders nothing is indistinguishable from
/// a broken build and gives the user nothing to correct.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub tiles: Vec<LaunchpadTile>,
    /// Columns and rows the drawing declared, which is simply its shape.
    pub columns: u8,
    pub rows: u8,
    /// Ready to print in the `look sources:` style, and to raise in the window
    /// (see the banner task) - this is the one config a GUI user edits and
    /// immediately looks at the result of, so stderr alone is invisible.
    pub warnings: Vec<String>,
}

impl Resolved {
    /// The built-in grid, used when there is no file and whenever one cannot be
    /// trusted as a whole.
    fn default_with(warnings: Vec<String>) -> Self {
        let tiles = look_qactions::launchpad_layout();
        let (columns, rows) = extent(&tiles);
        Self {
            tiles,
            columns,
            rows,
            warnings,
        }
    }
}

/// How far the placed tiles actually reach.
fn extent(tiles: &[LaunchpadTile]) -> (u8, u8) {
    let columns = tiles.iter().map(|t| t.col + t.col_span).max().unwrap_or(0);
    let rows = tiles.iter().map(|t| t.row + t.row_span).max().unwrap_or(0);
    (columns, rows)
}

/// The user's layout, with anything wrong with it printed.
///
/// Both shells call this rather than the catalog directly, so the drawing is
/// read once, in one place, and neither of them parses TOML or computes a span.
///
/// Printing here rather than at each call site keeps the stderr wording
/// identical across shells; the warnings come back as well, because stderr is
/// invisible to anyone who did not launch from a terminal (see the banner
/// task), and this is the one config a GUI user edits and immediately looks at
/// the result of.
pub fn layout_reported() -> Resolved {
    let resolved = layout();
    for warning in &resolved.warnings {
        eprintln!("look: {warning}");
    }
    resolved
}

/// The user's layout, or the built-in one.
///
/// Error policy, one rule borrowed from how sources already behave: a broken
/// file is skipped and reported rather than taking the system down.
///
/// > Structural errors fall back to the default layout. Per-tile errors drop
/// > that tile and keep the rest. The launchpad is never empty and never
/// > silent.
///
/// Falling back to the WHOLE default on a structural error is deliberate.
/// Rendering "as much as parsed" gives a launchpad that is wrong in a way the
/// user cannot diagnose by looking at it; the default is at least a known
/// state, and the warning says why they are seeing it.
pub fn layout() -> Resolved {
    let Some(home) = crate::config::user_home_dir() else {
        return Resolved::default_with(Vec::new());
    };
    let path = launchpad_path(Path::new(&home));

    // No file is the ordinary state of someone who never opened one, and of
    // every install between seeding failing and now. Not worth a word.
    if !path.exists() {
        return Resolved::default_with(Vec::new());
    }

    match std::fs::read_to_string(&path) {
        Ok(contents) => resolve(&contents),
        Err(err) => {
            Resolved::default_with(vec![format!("{} could not be read: {err}", path.display())])
        }
    }
}

/// The parse, split from the file so it can be tested without a home directory.
pub fn resolve(contents: &str) -> Resolved {
    let table: toml::Value = match toml::from_str(contents) {
        Ok(value) => value,
        Err(err) => {
            return Resolved::default_with(vec![format!(
                "launchpad.toml is not valid TOML, using the default layout: {err}"
            )]);
        }
    };

    let Some(drawn) = table.get("layout").and_then(|v| v.as_array()) else {
        return Resolved::default_with(vec![
            "launchpad.toml has no `layout` drawing, using the default layout".to_string(),
        ]);
    };

    let mut warnings = Vec::new();
    let mut grid: Vec<Vec<&str>> = Vec::new();
    for (index, line) in drawn.iter().enumerate() {
        let Some(text) = line.as_str() else {
            return Resolved::default_with(vec![format!(
                "launchpad.toml row {} is not a string, using the default layout",
                index + 1
            )]);
        };
        grid.push(text.split_whitespace().collect());
    }

    if grid.is_empty() || grid[0].is_empty() {
        return Resolved::default_with(vec![
            "launchpad.toml draws no tiles, using the default layout".to_string(),
        ]);
    }

    if grid.len() > MAX_ROWS {
        warnings.push(format!(
            "launchpad.toml draws {} rows; only the first {MAX_ROWS} are shown",
            grid.len()
        ));
        grid.truncate(MAX_ROWS);
    }

    // Ragged is structural: a row short of a token silently shifts everything
    // after it, so there is no partial reading of this that is safe to show.
    let columns = grid[0].len();
    if let Some(index) = grid.iter().position(|row| row.len() != columns) {
        return Resolved::default_with(vec![format!(
            "launchpad.toml row {} has {} tokens but row 1 has {columns}, using the default layout",
            index + 1,
            grid[index].len()
        )]);
    }

    let known: HashMap<String, LaunchpadTile> = look_qactions::launchpad_layout()
        .into_iter()
        .map(|tile| (tile.action_id.clone(), tile))
        .collect();

    // Bounding box per name. A cell holds one token, so two tiles claiming one
    // cell is not something this format can express.
    let mut boxes: HashMap<&str, (usize, usize, usize, usize)> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for (row, tokens) in grid.iter().enumerate() {
        for (col, name) in tokens.iter().enumerate() {
            if *name == HOLE {
                continue;
            }
            match boxes.get_mut(*name) {
                Some(area) => {
                    area.0 = area.0.min(col);
                    area.1 = area.1.min(row);
                    area.2 = area.2.max(col);
                    area.3 = area.3.max(row);
                }
                None => {
                    boxes.insert(name, (col, row, col, row));
                    order.push(name);
                }
            }
        }
    }

    let mut tiles = Vec::new();
    for name in order {
        let (c0, r0, c1, r1) = boxes[name];

        // Every cell of the bounding box must belong to this name. An L-shape
        // has a box like any other region, and placing it by that box would
        // silently cover cells the user gave to something else.
        let rectangular = (r0..=r1).all(|row| (c0..=c1).all(|col| grid[row][col] == name));
        if !rectangular {
            warnings.push(format!(
                "launchpad.toml: \"{name}\" must form a rectangle, so it is not shown"
            ));
            continue;
        }

        let Some(known) = known.get(name) else {
            warnings.push(format!(
                "launchpad.toml: \"{name}\" is not a tile Look knows, so it is not shown"
            ));
            continue;
        };

        tiles.push(LaunchpadTile {
            col: c0 as u8,
            row: r0 as u8,
            col_span: (c1 - c0 + 1) as u8,
            row_span: (r1 - r0 + 1) as u8,
            ..known.clone()
        });
    }

    if tiles.is_empty() {
        warnings.push("launchpad.toml placed no tiles, using the default layout".to_string());
        return Resolved::default_with(warnings);
    }

    // Reading order, so anything keyed off arrival - the shells' entrance
    // stagger, most of all - follows what is on screen rather than the order
    // names happened to appear in the file.
    tiles.sort_by_key(|tile| (tile.row, tile.col));

    Resolved {
        tiles,
        columns: columns as u8,
        rows: grid.len() as u8,
        warnings,
    }
}

const LAUNCHPAD_FILE_NAME: &str = ".look/launchpad.toml";

/// Where the layout lives. Beside `~/.look/config` rather than inside it: the
/// drawing is long enough to dominate a shared file, and it is the one config a
/// user edits repeatedly while arranging things.
///
/// It carries `.toml` even though `config` does not. The extension buys editor
/// syntax highlighting; `config` being extensionless is history, not a
/// convention worth extending.
pub fn launchpad_path(home: &Path) -> PathBuf {
    if let Ok(custom) = std::env::var(LAUNCHPAD_FILE_ENV) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home.join(LAUNCHPAD_FILE_NAME)
}

/// Writes the commented default on first run, the way `config.rs` seeds
/// `default_config_contents()`.
///
/// An existing file is never touched - not even to add keys it is missing, as
/// the config file does. A layout is a drawing the user arranged, and appending
/// to it would move their tiles.
///
/// Silent on failure, like the config seed: a home that cannot be written is
/// already reported by everything else that needs it, and a launchpad still
/// renders from the built-in default with no file at all.
pub fn ensure_default_file() {
    let Some(home) = crate::config::user_home_dir() else {
        return;
    };
    let path = launchpad_path(Path::new(&home));
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, DEFAULT_CONTENTS);
}

/// The seeded file. It reproduces today's layout exactly, so a fresh install
/// looks identical to an upgraded one, and the file IS the documentation -
/// there is nothing to look up before editing it, and "reset to default" is
/// `rm ~/.look/launchpad.toml`.
pub fn default_contents() -> &'static str {
    DEFAULT_CONTENTS
}

const DEFAULT_CONTENTS: &str = r#"# Your launchpad - the screen shown when the search bar is empty.
#
# Each line below is a row, and each token is one cell. Repeat a name across
# cells to make that tile span them; its cells must form a rectangle. Use "."
# for a deliberate gap. Every row needs the same number of tokens.
#
# There is no column or row count to declare: the drawing is the count. Delete
# this file to go back to these defaults.

layout = [
    "lslot       lslot       bluetooth   wifi        battery     weather",
    "lslot       lslot       theme       keepawake   screensaver weather",
    "mic         restart     shutdown    nowplaying  nowplaying  nowplaying",
]

# The built-in tiles, and the key that fires each one (with Cmd on macOS, Ctrl
# elsewhere):
#
#   lslot        the rotating Todo / Pomo / Clock slot    no key
#   bluetooth    toggle                                   B
#   wifi         toggle                                   W
#   battery      read-only level                          no key
#   theme        toggle, dark / light                     T
#   keepawake    toggle                                   K
#   screensaver  starts it                                S
#   weather      read-only condition + temperature        no key
#   mic          mute / unmute                            M
#   restart      asks first                               R
#   shutdown     asks first                               D
#   nowplaying   track name + play/pause                  P
#
# Deleting a name removes that tile and leaves a gap where it was - the layout
# is yours to arrange, so nothing closes up behind it.
#
# Up to 5 rows. More are ignored, with a warning on the next open.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(resolved: &Resolved, action_id: &str) -> Option<(u8, u8, u8, u8)> {
        resolved
            .tiles
            .iter()
            .find(|t| t.action_id == action_id)
            .map(|t| (t.col, t.row, t.col_span, t.row_span))
    }

    fn is_default(resolved: &Resolved) -> bool {
        resolved.tiles == look_qactions::launchpad_layout()
    }

    #[test]
    fn the_seeded_file_resolves_to_exactly_the_built_in_layout() {
        // The promise that makes this whole change invisible on upgrade: what
        // we write on first run must parse back to what we shipped.
        let resolved = resolve(DEFAULT_CONTENTS);
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert_eq!(resolved.tiles, look_qactions::launchpad_layout());
        assert_eq!((resolved.columns, resolved.rows), (6, 3));
    }

    #[test]
    fn a_repeated_name_spans_and_a_dot_leaves_a_hole() {
        let resolved = resolve(
            r#"layout = [
                "mic  mic   .",
                "mic  mic   theme",
            ]"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert_eq!(cells(&resolved, "mic"), Some((0, 0, 2, 2)));
        assert_eq!(cells(&resolved, "theme"), Some((2, 1, 1, 1)));
        // The hole is simply nothing placed there, not a tile of its own.
        assert_eq!(resolved.tiles.len(), 2);
        assert_eq!((resolved.columns, resolved.rows), (3, 2));
    }

    #[test]
    fn tiles_arrive_in_reading_order_whatever_order_the_file_names_them() {
        // The shells key their entrance stagger off arrival order, so this is
        // what stops a cascade from following the file instead of the screen.
        let resolved = resolve(
            r#"layout = [
                "shutdown  mic",
                "theme     wifi",
            ]"#,
        );
        let ids: Vec<&str> = resolved
            .tiles
            .iter()
            .map(|t| t.action_id.as_str())
            .collect();
        assert_eq!(ids, vec!["shutdown", "mic", "theme", "wifi"]);
    }

    #[test]
    fn a_tile_keeps_its_catalog_presentation_wherever_it_is_placed() {
        // Only geometry comes from the drawing. Title, role, mnemonic and the
        // toggle captions still come from the shared catalog, so moving a tile
        // cannot quietly restyle it or repoint its key.
        let resolved = resolve(r#"layout = ["theme"]"#);
        let theme = &resolved.tiles[0];
        let catalog = look_qactions::launchpad_layout()
            .into_iter()
            .find(|t| t.action_id == "theme")
            .expect("theme is a built-in");
        assert_eq!(theme.title, catalog.title);
        assert_eq!(theme.mnemonic, catalog.mnemonic);
        assert_eq!(theme.role, catalog.role);
        assert_eq!(theme.on_label, catalog.on_label);
    }

    // --- structural errors: the whole default, and say why ------------------

    #[test]
    fn invalid_toml_falls_back_to_the_whole_default() {
        let resolved = resolve("layout = [\"mic\"");
        assert!(is_default(&resolved));
        assert_eq!(resolved.warnings.len(), 1);
        assert!(resolved.warnings[0].contains("not valid TOML"));
    }

    #[test]
    fn a_file_with_no_drawing_falls_back_to_the_whole_default() {
        let resolved = resolve("columns = 6\n");
        assert!(is_default(&resolved));
        assert!(resolved.warnings[0].contains("no `layout`"));
    }

    #[test]
    fn a_ragged_drawing_falls_back_to_the_whole_default_and_names_the_row() {
        // Partial rendering is worse than the default here: one missing token
        // shifts every tile after it, and the user cannot see why by looking.
        let resolved = resolve(
            r#"layout = [
                "mic   theme  wifi",
                "mic   theme",
            ]"#,
        );
        assert!(is_default(&resolved));
        assert!(
            resolved.warnings[0].contains("row 2"),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn an_empty_drawing_falls_back_to_the_whole_default() {
        assert!(is_default(&resolve("layout = []")));
        assert!(is_default(&resolve(r#"layout = [""]"#)));
    }

    #[test]
    fn more_rows_than_the_cap_are_dropped_rather_than_refused() {
        let mut lines = vec![String::from("layout = [")];
        for _ in 0..MAX_ROWS + 2 {
            lines.push("    \"mic\",".to_string());
        }
        lines.push("]".to_string());
        let resolved = resolve(&lines.join("\n"));

        assert_eq!(resolved.rows as usize, MAX_ROWS);
        assert!(resolved.warnings[0].contains(&format!("first {MAX_ROWS}")));
        // Still a working launchpad, not a fallback: the drawing was legible,
        // it was merely taller than the window is laid out for.
        assert!(!is_default(&resolved));
    }

    // --- per-tile errors: drop that tile, keep the rest ---------------------

    #[test]
    fn an_unknown_name_leaves_its_cells_empty_and_the_rest_renders() {
        let resolved = resolve(
            r#"layout = [
                "mic  nosuchtile  theme",
            ]"#,
        );
        assert_eq!(resolved.tiles.len(), 2);
        assert!(cells(&resolved, "mic").is_some());
        assert!(cells(&resolved, "theme").is_some());
        assert!(
            resolved.warnings[0].contains("nosuchtile"),
            "{:?}",
            resolved.warnings
        );
        // The grid keeps the width the user drew: the hole is where the
        // unknown tile was, not closed up behind it.
        assert_eq!(resolved.columns, 3);
    }

    #[test]
    fn an_l_shaped_region_is_dropped_rather_than_placed_by_its_bounding_box() {
        // The one cost of the drawing, stated in the plan: a name's cells must
        // form a rectangle. Placing an L by its box would cover `theme`.
        let resolved = resolve(
            r#"layout = [
                "mic  mic",
                "mic  theme",
            ]"#,
        );
        assert!(cells(&resolved, "mic").is_none());
        assert_eq!(cells(&resolved, "theme"), Some((1, 1, 1, 1)));
        assert!(
            resolved.warnings[0].contains("rectangle"),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_drawing_that_places_nothing_falls_back_rather_than_rendering_empty() {
        // Every per-tile error at once. The rule is that the launchpad is never
        // empty, so the last resort is still the default.
        let resolved = resolve(r#"layout = ["nope  alsonope"]"#);
        assert!(is_default(&resolved));
        assert!(resolved.warnings.iter().any(|w| w.contains("nope")));
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("placed no tiles"))
        );
    }

    #[test]
    fn hiding_a_tile_is_deleting_its_name() {
        // The whole feature, in one assertion: four edits (hide, reorder,
        // resize, add) are all just editing the drawing.
        let resolved = resolve(
            r#"layout = [
                "mic    mic",
                "theme  .",
            ]"#,
        );
        assert!(cells(&resolved, "wifi").is_none(), "wifi was not drawn");
        assert_eq!(cells(&resolved, "mic"), Some((0, 0, 2, 1)));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    /// Every token of the drawing, row by row.
    fn drawing() -> Vec<Vec<&'static str>> {
        DEFAULT_CONTENTS
            .lines()
            .skip_while(|line| !line.starts_with("layout = ["))
            .skip(1)
            .take_while(|line| !line.starts_with(']'))
            .map(|line| {
                line.trim()
                    .trim_end_matches(',')
                    .trim_matches('"')
                    .split_whitespace()
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_seeded_drawing_is_the_layout_the_core_already_ships() {
        // The promise of this file: seeding it changes nothing. A fresh install
        // must look identical to one that upgraded into it, so the drawing has
        // to place every tile exactly where `launchpad_layout()` does.
        let rows = drawing();

        let mut drawn = std::collections::HashMap::new();
        for (row, tokens) in rows.iter().enumerate() {
            for (col, name) in tokens.iter().enumerate() {
                if *name == "." {
                    continue;
                }
                let cell = drawn.entry(*name).or_insert((col, row, col, row));
                cell.0 = cell.0.min(col);
                cell.1 = cell.1.min(row);
                cell.2 = cell.2.max(col);
                cell.3 = cell.3.max(row);
            }
        }

        for tile in look_qactions::launchpad_layout() {
            let (c0, r0, c1, r1) = *drawn
                .get(tile.action_id.as_str())
                .unwrap_or_else(|| panic!("{} is not in the seeded drawing", tile.action_id));
            assert_eq!(
                (c0 as u8, r0 as u8, (c1 - c0 + 1) as u8, (r1 - r0 + 1) as u8),
                (tile.col, tile.row, tile.col_span, tile.row_span),
                "{} sits somewhere else in the seeded file",
                tile.action_id
            );
        }

        assert_eq!(
            drawn.len(),
            look_qactions::launchpad_layout().len(),
            "the drawing names a tile the core does not ship, or vice versa"
        );
    }

    #[test]
    fn the_seeded_drawing_is_rectangular() {
        // A ragged drawing is the one structural error a user cannot see by
        // reading it, so the file we hand them must not model one.
        let rows = drawing();
        assert!(!rows.is_empty(), "the drawing has rows");
        assert!(rows.len() <= 5, "the seeded drawing is within the row cap");
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row.len(),
                rows[0].len(),
                "row {index} has a different number of tokens"
            );
        }
    }

    #[test]
    fn every_mnemonic_the_comment_claims_is_the_one_the_core_binds() {
        // The legend is documentation, and documentation that drifts is worse
        // than none: a user reads "D" and presses it expecting Shut Down.
        for tile in look_qactions::launchpad_layout() {
            let line = DEFAULT_CONTENTS
                .lines()
                .find(|line| {
                    line.trim_start()
                        .starts_with(&format!("#   {} ", tile.action_id))
                })
                .unwrap_or_else(|| panic!("{} has no legend line", tile.action_id));

            match tile.mnemonic {
                Some(key) => assert!(
                    line.trim_end().ends_with(key),
                    "the legend for {} does not end in {key}: {line}",
                    tile.action_id
                ),
                None => assert!(
                    line.trim_end().ends_with("no key"),
                    "{} has no mnemonic but its legend claims one: {line}",
                    tile.action_id
                ),
            }
        }
    }

    #[test]
    fn the_legend_columns_line_up() {
        // This file is the documentation, and it is read in a text editor. A
        // key one column out is invisible to a test that only checks the line
        // ENDS with it, and obvious to everyone who opens the file.
        let keys: Vec<usize> = DEFAULT_CONTENTS
            .lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("#   ")?;
                let name = rest.split_whitespace().next()?;
                look_qactions::launchpad_layout()
                    .iter()
                    .any(|t| t.action_id == name)
                    .then(|| line.rfind("  ").map(|i| i + 2))?
            })
            .collect();

        assert_eq!(keys.len(), 12, "one legend line per tile");
        assert!(
            keys.iter().all(|column| *column == keys[0]),
            "the key column is ragged: {keys:?}"
        );
    }

    /// Both halves in one test on purpose: `LAUNCHPAD_FILE_ENV` is process
    /// global, and two tests setting it would race under the parallel runner.
    #[test]
    fn seeding_writes_the_default_once_and_never_touches_it_again() {
        let dir = std::env::temp_dir().join(format!("look-launchpad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("launchpad.toml");
        let _ = std::fs::remove_file(&path);

        unsafe { std::env::set_var(LAUNCHPAD_FILE_ENV, &path) };

        // Absent: written.
        ensure_default_file();
        assert_eq!(
            std::fs::read_to_string(&path).expect("the file was seeded"),
            DEFAULT_CONTENTS
        );

        // Present and edited: left alone. Not merely "not overwritten" - the
        // config file APPENDS keys it is missing on every load, and doing that
        // to a drawing would move the user's tiles.
        let theirs = "layout = [\"mic\"]\n";
        std::fs::write(&path, theirs).expect("write");
        ensure_default_file();
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            theirs,
            "seeding changed a layout the user had arranged"
        );

        unsafe { std::env::remove_var(LAUNCHPAD_FILE_ENV) };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
