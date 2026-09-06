//! The user's Super Actions layout, `~/.look/super-actions.toml`.
//!
//! Named `launchpad` throughout the code, which is the internal word for this
//! screen; `Super Actions` is what Settings and the docs call it.
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
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Overrides the file, for tests and for anyone running two configurations.
pub const LAUNCHPAD_FILE_ENV: &str = "LOOK_LAUNCHPAD_FILE";

/// The launchpad is the empty state of a window that has to appear instantly,
/// so its height is laid out against a known maximum rather than an open-ended
/// one. Three is today's layout; five leaves room for a band of your own tiles.
pub const MAX_ROWS: usize = 5;

/// The strip is one panel wide, so past six the tiles stop being legible. Also
/// keeps every coordinate inside the `u8` it crosses the FFI as.
pub const MAX_COLUMNS: usize = 6;

/// A gap the user drew on purpose.
const HOLE: &str = ".";

/// What a tile's `value` command prints: one JSON object.
///
/// JSON, not a tab-separated line, because a built-in is not one line either -
/// Weather has a temperature, a condition, a high/low and a humidity. A user
/// tile sits beside those. Named fields also age better than positions.
///
/// The same shape in and out, so what a script writes is what is documented.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, Serialize)]
pub struct TileValue {
    /// The large text. "29°", "98%", "in 24m".
    pub value: String,
    /// The small line under it, drawn the way BATTERY and PARTLY CLOUDY are.
    pub caption: Option<String>,
    /// Anything further, one per line, shown by a tile with the room for it.
    #[serde(default)]
    pub lines: Vec<String>,
    /// An icon name, when the tile wants one of its own.
    pub icon: Option<String>,
    /// `"on"` / `"off"` for a tile that reads as a toggle, so it can take the
    /// same active treatment the built-in toggles use.
    pub state: Option<String>,
}

/// A tile the user declared, from a `[tiles.<name>]` entry.
///
/// Declared whole here, so `~/.look/sources/` is untouched and "why is my tile
/// blank" is one file to read. The commands never reach the wire: a shell
/// renders a value the core resolved and cannot be the thing that runs it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileDef {
    /// What it displays, as JSON. `None` for a tile that only acts - Mic and
    /// Screensaver are built-ins of that shape. Such a tile never spawns.
    pub value: Option<String>,
    /// How stale that line may get before it is asked for again.
    pub refresh: Option<Duration>,
    /// What pressing it runs. A shell command, like `value`.
    ///
    /// Not a block in `~/.look/sources/`: that split put a tile's behaviour
    /// back across two files, and invented a failure where an uninstalled
    /// block renders a tile that silently does nothing. `confirm` is what the
    /// link was buying.
    pub press: Option<String>,
    /// Asked before `press` runs, so a tile that changes something can say so.
    /// Empty or absent means it just runs.
    pub confirm: Option<String>,
    /// What it is called. The drawing's token is an id, so without this the
    /// name is title-cased into a label.
    pub title: Option<String>,
    /// The symbol drawn on it, and the only route to one for a tile that just
    /// acts. An `icon` in a `value`'s JSON wins, changing with what was read.
    pub icon: Option<String>,
    /// The key that fires it, with Cmd / Alt. Requested, not granted: a
    /// built-in's letter is never given away.
    pub mnemonic: Option<char>,
}

/// A user tile, with its mnemonic granted or refused.
///
/// - A letter held by a tile on the screen is never given away: a config file
///   must not repoint `Cmd+D` away from Shut Down.
/// - `Q` is refused - the menu takes it before Look's monitor runs, so the tile
///   would silently never fire.
/// - First drawn wins between two user tiles.
/// - Case-insensitive, as both shells match.
///
/// A refusal keeps the tile; the key is a shortcut to something still clickable.
fn custom_tile(
    name: &str,
    def: &TileDef,
    claimed: &mut HashMap<char, String>,
    warnings: &mut Vec<String>,
) -> LaunchpadTile {
    let title = def.title.clone().unwrap_or_else(|| title_cased(name));

    let mnemonic = def.mnemonic.and_then(|key| {
        if let Some(owner) = look_qactions::chord_owner(key) {
            warnings.push(format!(
                "super-actions.toml: \"{name}\" wants Cmd+{key}, which belongs to {owner} before a \
                 tile ever sees it, so it has no key"
            ));
            return None;
        }
        match claimed.get(&key) {
            Some(owner) => {
                warnings.push(format!(
                    "super-actions.toml: \"{name}\" wants Cmd+{key}, which belongs to \"{owner}\", \
                     so it has no key"
                ));
                None
            }
            None => {
                claimed.insert(key, name.to_string());
                Some(key)
            }
        }
    });

    // A letter the title does not contain still fires, but neither shell can
    // highlight it, so nobody would ever discover it.
    if let Some(key) = mnemonic
        && !title.to_ascii_uppercase().contains(key)
    {
        warnings.push(format!(
            "super-actions.toml: \"{name}\" has mnemonic \"{key}\", which is not in its title \
             \"{title}\", so the key works but is not shown on the tile"
        ));
    }

    LaunchpadTile {
        action_id: name.to_string(),
        title,
        // Presentation only. A user tile is dressed like the small controls;
        // how much room it gets is the drawing's business, not this.
        size: look_qactions::TileSize::S,
        role: look_qactions::TileRole::Custom,
        mnemonic,
        col: 0,
        row: 0,
        col_span: 1,
        row_span: 1,
        on_label: None,
        off_label: None,
        // A tile with no `press` is a readout, like Battery.
        pressable: def.press.is_some(),
        has_value: def.value.is_some(),
        confirm: def.confirm.clone(),
        icon: def.icon.clone(),
    }
}

/// `meeting-next` -> `Meeting Next`. The drawing's token is an id; a tile still
/// needs something to call itself.
fn title_cased(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every `[tiles.<name>]` entry, and a complaint for each one that cannot be
/// used. A legend entry is only consulted for a name the drawing places, so an
/// entry for a tile nobody drew is inert rather than an error - which is how a
/// user keeps a tile's definition around while trying the grid without it.
fn legend(table: &toml::Value) -> (HashMap<String, TileDef>, Vec<String>) {
    let mut defs = HashMap::new();
    let mut warnings = Vec::new();

    let Some(entries) = table.get("tiles").and_then(|v| v.as_table()) else {
        return (defs, warnings);
    };

    for (name, entry) in entries {
        let Some(entry) = entry.as_table() else {
            warnings.push(format!("super-actions.toml: [tiles.{name}] is not a table"));
            continue;
        };

        let str_key = |key: &str| {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };

        let value = str_key("value");
        let press = str_key("press");

        // A tile that neither shows anything nor does anything is a blank card
        // the user cannot diagnose by looking at it. One or the other is enough:
        // `value` alone is a readout, `press` alone is a button.
        if value.is_none() && press.is_none() {
            warnings.push(format!(
                "super-actions.toml: [tiles.{name}] has neither a `value` to show nor a `press` to \
                 run, so it is not shown"
            ));
            continue;
        }

        let confirm = str_key("confirm");
        let icon = str_key("icon");
        // A question nothing ever asks. Not fatal - the tile still shows its
        // value - but the user wrote it expecting to be asked something.
        if confirm.is_some() && press.is_none() {
            warnings.push(format!(
                "super-actions.toml: [tiles.{name}] has a `confirm` but no `press`, so nothing asks it"
            ));
        }

        let refresh = entry
            .get("refresh")
            .and_then(|v| v.as_str())
            .and_then(|raw| {
                look_sources::parse_duration(raw).ok().or_else(|| {
                    warnings.push(format!(
                        "super-actions.toml: [tiles.{name}] has refresh = \"{raw}\", which is not a \
                         duration like \"60s\", \"5m\" or \"2h\""
                    ));
                    None
                })
            });

        // One character, so a two-letter request is a mistake worth naming
        // rather than silently taking the first letter of.
        let mnemonic = entry
            .get("mnemonic")
            .and_then(|v| v.as_str())
            .and_then(|raw| {
                let mut chars = raw.trim().chars();
                match (chars.next(), chars.next()) {
                    (Some(key), None) => Some(key.to_ascii_uppercase()),
                    _ => {
                        warnings.push(format!(
                            "super-actions.toml: [tiles.{name}] has mnemonic = \"{raw}\", which is \
                             not a single character"
                        ));
                        None
                    }
                }
            });

        defs.insert(
            name.clone(),
            TileDef {
                value,
                icon,
                refresh,
                press,
                confirm,
                title: str_key("title"),
                mnemonic,
            },
        );
    }

    (defs, warnings)
}

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
    /// What each user tile actually runs, keyed by the name in the drawing.
    ///
    /// Kept beside the tiles rather than inside them: a `LaunchpadTile` is the
    /// wire format, and a shell has no business knowing the command behind a
    /// value it only renders. Only entries for tiles that were actually placed
    /// appear here.
    pub defs: HashMap<String, TileDef>,
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
            // The built-in grid is all built-in tiles, so nothing to run.
            defs: HashMap::new(),
        }
    }
}

/// How far the placed tiles actually reach.
fn extent(tiles: &[LaunchpadTile]) -> (u8, u8) {
    let columns = tiles.iter().map(|t| t.col + t.col_span).max().unwrap_or(0);
    let rows = tiles.iter().map(|t| t.row + t.row_span).max().unwrap_or(0);
    (columns, rows)
}

/// What a shell decodes. The shape travels rather than being derived from how
/// far the tiles reach, which cannot see a trailing empty track: `mic . .`
/// would come out one column wide, stretched across the strip.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutPayload {
    pub columns: u8,
    pub rows: u8,
    pub tiles: Vec<LaunchpadTile>,
}

impl From<Resolved> for LayoutPayload {
    fn from(resolved: Resolved) -> Self {
        Self {
            columns: resolved.columns,
            rows: resolved.rows,
            tiles: resolved.tiles,
        }
    }
}

/// `layout_reported`, in the shape both bridges hand their shell.
pub fn layout_payload() -> LayoutPayload {
    layout_reported().into()
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
    let home = Path::new(&home);
    let mut resolved = layout_at(&launchpad_path(home));
    if let Some(warning) = legacy_file_warning(home) {
        resolved.warnings.push(warning);
    }
    resolved
}

/// The file this one used to be called. Nothing reads it, so an edit there
/// looks like Look ignoring the layout. Never shipped under that name, so there
/// is nothing to migrate - only something to say.
const LEGACY_FILE_NAME: &str = ".look/launchpad.toml";

fn legacy_file_warning(home: &Path) -> Option<String> {
    // A caller that named its own file has already said which one it means.
    if std::env::var_os(LAUNCHPAD_FILE_ENV).is_some_and(|value| !value.is_empty()) {
        return None;
    }
    home.join(LEGACY_FILE_NAME).exists().then(|| {
        format!("~/{LEGACY_FILE_NAME} is no longer read - the layout now lives in ~/{LAUNCHPAD_FILE_NAME}")
    })
}

fn layout_at(path: &Path) -> Resolved {
    // No file is the ordinary state of someone who never opened one, and of
    // every install between seeding failing and now. Not worth a word.
    if !path.exists() {
        return Resolved::default_with(Vec::new());
    }

    // Memoized on the file's mtime. A launcher open asks for this up to three
    // times - the layout, its warnings, and the tile refresh - and the seeded
    // file is 4 KB of mostly comments, so without this every open reparses it
    // twice over to reach the same answer. An edit changes the mtime, so the
    // reload loop still sees it immediately.
    let stamp = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    let cache = memo().lock().unwrap_or_else(|err| err.into_inner());
    if let Some((cached_stamp, cached)) = &*cache
        && stamp.is_some()
        && *cached_stamp == stamp
    {
        return cached.clone();
    }
    drop(cache);

    let resolved = match std::fs::read_to_string(path) {
        Ok(contents) => resolve(&contents),
        Err(err) => {
            Resolved::default_with(vec![format!("{} could not be read: {err}", path.display())])
        }
    };

    if stamp.is_some() {
        *memo().lock().unwrap_or_else(|err| err.into_inner()) = Some((stamp, resolved.clone()));
    }
    resolved
}

/// The last parse, and the mtime it was parsed from.
type Memo = Option<(Option<std::time::SystemTime>, Resolved)>;

fn memo() -> &'static std::sync::Mutex<Memo> {
    static MEMO: std::sync::OnceLock<std::sync::Mutex<Memo>> = std::sync::OnceLock::new();
    MEMO.get_or_init(|| std::sync::Mutex::new(None))
}

/// The parse, split from the file so it can be tested without a home directory.
pub fn resolve(contents: &str) -> Resolved {
    let table: toml::Value = match toml::from_str(contents) {
        Ok(value) => value,
        Err(err) => {
            return Resolved::default_with(vec![format!(
                "super-actions.toml is not valid TOML, using the default layout: {err}"
            )]);
        }
    };

    let Some(drawn) = table.get("layout").and_then(|v| v.as_array()) else {
        return Resolved::default_with(vec![
            "super-actions.toml has no `layout` drawing, using the default layout".to_string(),
        ]);
    };

    let mut warnings = Vec::new();
    let mut grid: Vec<Vec<&str>> = Vec::new();
    for (index, line) in drawn.iter().enumerate() {
        let Some(text) = line.as_str() else {
            return Resolved::default_with(vec![format!(
                "super-actions.toml row {} is not a string, using the default layout",
                index + 1
            )]);
        };
        grid.push(text.split_whitespace().collect());
    }

    if grid.is_empty() || grid[0].is_empty() {
        return Resolved::default_with(vec![
            "super-actions.toml draws no tiles, using the default layout".to_string(),
        ]);
    }

    if grid.len() > MAX_ROWS {
        warnings.push(format!(
            "super-actions.toml draws {} rows; only the first {MAX_ROWS} are shown",
            grid.len()
        ));
        grid.truncate(MAX_ROWS);
    }

    // Ragged is structural: a row short of a token silently shifts everything
    // after it, so there is no partial reading of this that is safe to show.
    let mut columns = grid[0].len();
    if let Some(index) = grid.iter().position(|row| row.len() != columns) {
        return Resolved::default_with(vec![format!(
            "super-actions.toml row {} has {} tokens but row 1 has {columns}, using the default layout",
            index + 1,
            grid[index].len()
        )]);
    }

    if columns > MAX_COLUMNS {
        warnings.push(format!(
            "super-actions.toml draws {columns} columns; only the first {MAX_COLUMNS} are shown"
        ));
        for row in &mut grid {
            row.truncate(MAX_COLUMNS);
        }
        columns = MAX_COLUMNS;
    }

    let (defs, legend_warnings) = legend(&table);
    warnings.extend(legend_warnings);

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

    // Every letter already spoken for, and who has it.
    //
    // Seeded from the built-ins the drawing actually PLACES, not from the whole
    // catalog. A key only ever fires while the launchpad is on screen, so one
    // belonging to a tile the user took off it does nothing - reserving it
    // would deny them a letter to protect a tile that is not there. Put the
    // tile back and it takes its letter back, with the collision reported.
    //
    // Case-insensitive, because both shells match that way: `n` and `N` are one
    // key.
    let mut claimed: HashMap<char, String> = order
        .iter()
        .filter_map(|name| known.get(*name))
        .filter_map(|tile| {
            tile.mnemonic
                .map(|key| (key.to_ascii_uppercase(), tile.action_id.clone()))
        })
        .collect();

    let mut tiles = Vec::new();
    for name in order {
        let (c0, r0, c1, r1) = boxes[name];

        // Every cell of the bounding box must belong to this name. An L-shape
        // has a box like any other region, and placing it by that box would
        // silently cover cells the user gave to something else.
        let rectangular = (r0..=r1).all(|row| (c0..=c1).all(|col| grid[row][col] == name));
        if !rectangular {
            warnings.push(format!(
                "super-actions.toml: \"{name}\" must form a rectangle, so it is not shown"
            ));
            continue;
        }

        let col_span = (c1 - c0 + 1) as u8;
        let row_span = (r1 - r0 + 1) as u8;

        // A bare name is a built-in. A name with a `[tiles.<name>]` entry is
        // the user's own, defined entirely there. The common case - moving a
        // built-in around - needs no legend at all.
        let known = match known.get(name) {
            Some(built_in) => {
                // The built-in wins: its letter and press are Look's to define.
                // Said out loud, or the entry's `value` runs on every refresh
                // and renders nowhere.
                if defs.contains_key(name) {
                    warnings.push(format!(
                        "super-actions.toml: \"{name}\" is a built-in tile, so its [tiles.{name}] \
                         entry is ignored"
                    ));
                }
                built_in.clone()
            }
            None => {
                let Some(def) = defs.get(name) else {
                    warnings.push(format!(
                        "super-actions.toml: \"{name}\" is not a tile Look knows and has no \
                         [tiles.{name}] entry, so it is not shown"
                    ));
                    continue;
                };
                // A `value` is read back from a file named after the tile, so a
                // name that cannot be a filename could never show one. Refused
                // here, where there is somewhere to say so.
                if def.value.is_some() && !crate::launchpad_values::is_cacheable_name(name) {
                    warnings.push(format!(
                        "super-actions.toml: \"{name}\" has a `value` to show but its name cannot \
                         be cached (letters, digits, - and _ only), so it is not shown"
                    ));
                    continue;
                }
                custom_tile(name, def, &mut claimed, &mut warnings)
            }
        };

        // Dropped rather than grown: growing would cover cells given to
        // another tile, and a tile under its minimum clips rather than shrinks.
        let (min_col, min_row) = look_qactions::min_span(known.role);
        if col_span < min_col || row_span < min_row {
            warnings.push(format!(
                "super-actions.toml: \"{name}\" needs at least {min_col}x{min_row} cells but is \
                 drawn {col_span}x{row_span}, so it is not shown"
            ));
            continue;
        }

        tiles.push(LaunchpadTile {
            col: c0 as u8,
            row: r0 as u8,
            col_span,
            row_span,
            ..known.clone()
        });
    }

    if tiles.is_empty() {
        warnings.push("super-actions.toml placed no tiles, using the default layout".to_string());
        return Resolved::default_with(warnings);
    }

    // Reading order, so anything keyed off arrival - the shells' entrance
    // stagger, most of all - follows what is on screen rather than the order
    // names happened to appear in the file.
    tiles.sort_by_key(|tile| (tile.row, tile.col));

    // Only what was placed. A legend entry for a tile the drawing does not name
    // is inert by design - it is how a user keeps a definition around while
    // trying the grid without it - and carrying it here would have the cache
    // run a command for a tile that is not on screen.
    let placed: HashMap<String, TileDef> = tiles
        .iter()
        .filter(|tile| tile.role == look_qactions::TileRole::Custom)
        .filter_map(|tile| {
            defs.get(&tile.action_id)
                .map(|def| (tile.action_id.clone(), def.clone()))
        })
        .collect();

    Resolved {
        tiles,
        columns: columns as u8,
        rows: grid.len() as u8,
        warnings,
        defs: placed,
    }
}

const LAUNCHPAD_FILE_NAME: &str = ".look/super-actions.toml";

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
/// `rm ~/.look/super-actions.toml`.
pub fn default_contents() -> &'static str {
    DEFAULT_CONTENTS
}

const DEFAULT_CONTENTS: &str = r#"# Your Super Actions strip - the screen shown when the search bar is empty.
#
# Each line below is a row, and each token is one cell. Repeat a name across
# cells to make that tile span them; its cells must form a rectangle. Use "."
# for a deliberate gap. Every row needs the same number of tokens.
#
# There is no column or row count to declare: the drawing is the count, up to
# 5 rows and 6 columns. Delete this file to go back to these defaults.

layout = [
    "lslot       lslot       bluetooth   wifi        battery     weather",
    "lslot       lslot       theme       keepawake   screensaver weather",
    "mic         restart     shutdown    nowplaying  nowplaying  nowplaying",
]

# The built-in tiles, the key that fires each one (with Cmd on macOS, Alt
# elsewhere), and the smallest area it can be drawn in:
#
#   lslot        the rotating Todo / Pomo / Clock slot    no key  2x2
#   bluetooth    toggle                                   B       1x1
#   wifi         toggle                                   W       1x1
#   battery      read-only level                          no key  1x1
#   theme        toggle, dark / light                     T       1x1
#   keepawake    toggle                                   K       1x1
#   screensaver  starts it                                S       1x1
#   weather      read-only condition + temperature        no key  1x2
#   mic          mute / unmute                            M       1x1
#   restart      asks first                               R       1x1
#   shutdown     asks first                               D       1x1
#   nowplaying   track name + play/pause                  P       2x1
#
# Columns x rows. Bigger is fine; smaller is left out with a warning, since a
# tile under its minimum clips rather than shrinks.
#
# Deleting a name removes that tile and leaves a gap where it was - the layout
# is yours to arrange, so nothing closes up behind it.
#
# Up to 5 rows and 6 columns. More are ignored, with a warning on the next open.


# ---------------------------------------------------------------- your own --
#
# A tile of your own is a name in the drawing above plus an entry here. Nothing
# needs to change in ~/.look/sources/ - a tile is declared whole, right here.
#
#     layout = [
#         "lslot       lslot       bluetooth   wifi        battery     weather",
#         "lslot       lslot       theme       keepawake   screensaver weather",
#         "meeting     meeting     shutdown    nowplaying  nowplaying  nowplaying",
#     ]
#
#     [tiles.meeting]
#     value    = "~/.look/bin/meeting-next"    # prints the JSON below
#     refresh  = "60s"                         # how stale it may get
#     press    = "~/.look/bin/meeting-join"    # optional: what pressing it runs
#     confirm  = "Join the meeting?"           # optional: asked before press
#     title    = "Next up"                     # optional: else the name, cased
#     icon     = "calendar"                    # optional: the symbol drawn on it
#     mnemonic = "N"                           # optional: Cmd+N / Alt+N
#
# `value` prints one JSON object. Only `value` is required, so a one-liner is a
# whole tile:
#
#     {"value": "in 24m"}
#
# A tile with room can say as much as Weather does:
#
#     {"value":   "in 24m",
#      "caption": "PLATFORM WEEKLY",
#      "lines":   ["3 attendees", "Zoom"],
#      "icon":    "calendar",
#      "state":   "on"}
#
# The caption and the extra lines only show on a tile drawn bigger than one
# cell, because there is nowhere to put them otherwise.
#
# Printing nothing hides the tile: that is how a "next meeting" tile disappears
# on a day with no meetings.
#
# Keep `value` light. It runs unattended, so it is capped at two seconds and
# 16 KB - past that it is killed, along with anything it started, and the tile
# keeps its last good reading. Read something and print it. Anything that has to
# fetch, build or wait belongs behind `press`, or in a script that caches to a
# file this only reads.
#
# A letter already used by a tile on the screen is not given away, and Cmd+Q
# belongs to quitting Look. Either way the tile still works, it just has no key,
# and the reason is reported on the next open.
"#;

// ---------------------------------------------------------------- writing --
//
// The drawing is edited by hand, and now also by dragging a tile on macOS.
// The second route ends here: the shell reports where every tile landed, this
// draws that as the same strings a person would type, and only the `layout`
// array changes. Anything else in the file - the comments the seed ships, a
// `[tiles.<name>]` entry, a blank line - is the user's and stays.

/// One tile as a shell hands it back after a drag: the name in the drawing and
/// the cells it now covers. The same numbers `LaunchpadTile` carries out, so a
/// shell reports what it was given, moved, and never invents a size.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Placement {
    pub action_id: String,
    pub col: u8,
    pub row: u8,
    pub col_span: u8,
    pub row_span: u8,
}

impl From<&LaunchpadTile> for Placement {
    fn from(tile: &LaunchpadTile) -> Self {
        Self {
            action_id: tile.action_id.clone(),
            col: tile.col,
            row: tile.row,
            col_span: tile.col_span,
            row_span: tile.row_span,
        }
    }
}

/// The drawing for `placements` on a `columns` x `rows` grid: one string per
/// row, a hole wherever nothing is placed, tokens padded so the columns line
/// up the way the seeded file's do. `Err` names the first thing that cannot be
/// drawn - two tiles on one cell, a tile past the edge, a name the tokenizer
/// would split - because a drawing with any of those would read back as
/// something other than what was dropped.
pub fn draw(placements: &[Placement], columns: u8, rows: u8) -> Result<Vec<String>, String> {
    let (columns, rows) = (columns as usize, rows as usize);
    if columns == 0 || rows == 0 || columns > MAX_COLUMNS || rows > MAX_ROWS {
        return Err(format!(
            "a drawing is 1 to {MAX_COLUMNS} columns by 1 to {MAX_ROWS} rows, not {columns}x{rows}"
        ));
    }
    // Refused rather than written as a grid of dots: the file would then read
    // "placed no tiles" and fall back to the default, which is not what an
    // empty arrangement asked for. Deleting the file is how that is said.
    if placements.is_empty() {
        return Err("an arrangement with no tiles is not saved".to_string());
    }

    let mut grid: Vec<Vec<&str>> = vec![vec![HOLE; columns]; rows];
    for placement in placements {
        let name = placement.action_id.as_str();
        // `resolve` splits rows on whitespace, so a name with any in it would
        // come back as two tiles. Never true of a name the resolver produced.
        if name.is_empty() || name == HOLE || name.split_whitespace().ne([name]) {
            return Err(format!("\"{name}\" cannot be a name in the drawing"));
        }
        if placement.col_span == 0 || placement.row_span == 0 {
            return Err(format!("\"{name}\" covers no cells"));
        }
        let (c0, r0) = (placement.col as usize, placement.row as usize);
        let (c1, r1) = (
            c0 + placement.col_span as usize,
            r0 + placement.row_span as usize,
        );
        if c1 > columns || r1 > rows {
            return Err(format!("\"{name}\" reaches past the {columns}x{rows} grid"));
        }
        for (row, cells) in grid.iter_mut().enumerate().take(r1).skip(r0) {
            for (col, cell) in cells.iter_mut().enumerate().take(c1).skip(c0) {
                if *cell != HOLE {
                    return Err(format!(
                        "\"{name}\" and \"{cell}\" both claim column {}, row {}",
                        col + 1,
                        row + 1
                    ));
                }
                *cell = name;
            }
        }
    }

    // One width for every column - the longest name plus a space - which is
    // how the seed is laid out, so the default arrangement writes back as the
    // seed, byte for byte. The last token on a row is not padded: trailing
    // spaces are invisible and travel badly through editors.
    let width = grid.iter().flatten().map(|t| t.len()).max().unwrap_or(0) + 1;
    Ok(grid
        .iter()
        .map(|row| {
            let mut line = String::new();
            for (index, token) in row.iter().enumerate() {
                if index + 1 == row.len() {
                    line.push_str(token);
                } else {
                    line.push_str(&format!("{token:<width$}"));
                }
            }
            line
        })
        .collect())
}

/// `contents` with its `layout` replaced by `drawing` and nothing else
/// touched: every comment and every `[tiles.<name>]` entry stays where it
/// was, because the file is the user's and the seeded one is its own
/// documentation. A file this cannot parse is left alone - rewriting it would
/// trade a broken drawing the user can fix for a lost one.
pub fn with_drawing(contents: &str, drawing: &[String]) -> Result<String, String> {
    let mut document = contents.parse::<toml_edit::DocumentMut>().map_err(|err| {
        format!("super-actions.toml is not valid TOML, so the arrangement was not saved: {err}")
    })?;

    let mut rows = toml_edit::Array::new();
    for line in drawing {
        let mut value = toml_edit::Value::from(line.as_str());
        value.decor_mut().set_prefix("\n    ");
        rows.push_formatted(value);
    }
    rows.set_trailing_comma(true);
    rows.set_trailing("\n");

    // A root key, so a file that lost its `layout` gets one back ahead of its
    // tables, where the resolver looks for it.
    document["layout"] = toml_edit::value(rows);
    Ok(document.to_string())
}

/// Every name the drawing in `contents` places, in first-seen order. Empty for
/// a file with no readable drawing: there is nothing there to lose.
fn drawn_names(contents: &str) -> Vec<String> {
    let Ok(table) = toml::from_str::<toml::Value>(contents) else {
        return Vec::new();
    };
    let Some(rows) = table.get("layout").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for token in rows
        .iter()
        .filter_map(|row| row.as_str())
        .flat_map(str::split_whitespace)
    {
        if token != HOLE && !names.iter().any(|name| name == token) {
            names.push(token.to_string());
        }
    }
    names
}

/// Writes the drawing for `placements` into `~/.look/super-actions.toml`.
/// `Err` is worded for a banner: this is the one write a user makes by hand
/// gesture and then looks at the result of.
pub fn save_layout(placements: &[Placement], columns: u8, rows: u8) -> Result<(), String> {
    let Some(home) = crate::config::user_home_dir() else {
        return Err("no home directory to save the arrangement in".to_string());
    };
    save_layout_at(&launchpad_path(Path::new(&home)), placements, columns, rows)
}

/// `save_layout` against a named file. Seeds from the default when there is
/// none, so a user who deleted the file to reset gets the comments back along
/// with their first drag.
///
/// Read back before it is written: `resolve` is the only judge of what a
/// drawing means, and a placement it would drop or move - a tile under its
/// minimum, say - is a save the user would see as Look losing a tile. Refused
/// here, with the resolver's own reason, while the file is still untouched.
pub fn save_layout_at(
    path: &Path,
    placements: &[Placement],
    columns: u8,
    rows: u8,
) -> Result<(), String> {
    let drawing = draw(placements, columns, rows)?;
    let theirs = path
        .exists()
        .then(|| std::fs::read_to_string(path))
        .transpose()
        .map_err(|err| format!("{} could not be read: {err}", path.display()))?;

    // What is on screen is what gets drawn, so a name their file has and the
    // strip does not - a tile the resolver dropped, with a warning - would be
    // erased by this write. Refused instead: the user was told the tile is
    // wrong, and losing it from the drawing is not the fix. The seed is not
    // theirs, so a missing file is not held to this.
    if let Some(theirs) = &theirs {
        let placed: std::collections::HashSet<&str> = placements
            .iter()
            .map(|placement| placement.action_id.as_str())
            .collect();
        if let Some(missing) = drawn_names(theirs)
            .into_iter()
            .find(|name| !placed.contains(name.as_str()))
        {
            return Err(format!(
                "the arrangement was not saved: \"{missing}\" is in the drawing but not on the \
                 strip - fix it or take it out of ~/.look/super-actions.toml first"
            ));
        }
    }
    let current = theirs.unwrap_or_else(|| DEFAULT_CONTENTS.to_string());

    let next = with_drawing(&current, &drawing)?;

    let resolved = resolve(&next);
    for wanted in placements {
        let landed = resolved.tiles.iter().any(|tile| {
            tile.action_id == wanted.action_id
                && tile.col == wanted.col
                && tile.row == wanted.row
                && tile.col_span == wanted.col_span
                && tile.row_span == wanted.row_span
        });
        if !landed {
            let quoted = format!("\"{}\"", wanted.action_id);
            let why = resolved
                .warnings
                .iter()
                .find(|warning| warning.contains(&quoted))
                .cloned()
                .unwrap_or_else(|| format!("{quoted} would not be shown where it was dropped"));
            return Err(format!("the arrangement was not saved: {why}"));
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("{} could not be created: {err}", parent.display()))?;
    }
    // Whole or not at all: a crash mid-write must not leave half a drawing for
    // the next open to fall back from.
    let staging = path.with_extension("toml.tmp");
    std::fs::write(&staging, &next)
        .map_err(|err| format!("{} could not be written: {err}", staging.display()))?;
    std::fs::rename(&staging, path)
        .map_err(|err| format!("{} could not be replaced: {err}", path.display()))?;

    // The memo keys on mtime, which has moved; cleared anyway so a write that
    // lands inside the clock's granularity cannot serve the old drawing.
    *memo().lock().unwrap_or_else(|err| err.into_inner()) = None;
    Ok(())
}

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

    fn tile<'a>(resolved: &'a Resolved, action_id: &str) -> Option<&'a LaunchpadTile> {
        resolved.tiles.iter().find(|t| t.action_id == action_id)
    }

    // --- user tiles ---------------------------------------------------------

    /// The drawing wins and the entry is refused out loud. Silence here meant a
    /// `value` running on every refresh into a tile that could never show it.
    #[test]
    fn a_legend_entry_for_a_built_in_is_refused_rather_than_obeyed() {
        let resolved = resolve(
            r#"
layout = ["mic  wifi"]

[tiles.mic]
value = "echo never-runs"
press = "echo never-runs"
"#,
        );

        let mic = tile(&resolved, "mic").expect("the drawn tile");
        assert_eq!(
            mic.role,
            look_qactions::TileRole::Action,
            "still the built-in"
        );
        assert!(
            !resolved.defs.contains_key("mic"),
            "a built-in must not carry a command into the value cache"
        );
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("mic") && w.contains("built-in")),
            "{:?}",
            resolved.warnings
        );
    }

    /// The value cache already refuses such a name; this proves the refusal
    /// reaches the user rather than showing as a tile that never fills in.
    #[test]
    fn a_value_whose_name_cannot_be_cached_is_refused_rather_than_left_blank() {
        let resolved = resolve(
            r#"
layout = ["ci/status  mic"]

[tiles."ci/status"]
value = "echo hi"
"#,
        );

        assert!(tile(&resolved, "ci/status").is_none(), "not shown");
        assert!(
            !resolved.defs.contains_key("ci/status"),
            "nothing for the cache to fail at later"
        );
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("ci/status") && w.contains("cached")),
            "{:?}",
            resolved.warnings
        );
        assert!(tile(&resolved, "mic").is_some(), "the rest still renders");

        // Only a `value` needs a filename. A button caches nothing, so its name
        // is never written anywhere and the drawing is free to say it.
        let button = resolve(
            r#"
layout = ["ci/status  mic"]

[tiles."ci/status"]
press = "echo hi"
"#,
        );
        assert!(button.warnings.is_empty(), "{:?}", button.warnings);
        assert!(tile(&button, "ci/status").is_some());
    }

    #[test]
    fn a_name_with_a_legend_entry_becomes_a_tile_of_the_users_own() {
        let resolved = resolve(
            r#"
layout = ["meeting  mic"]

[tiles.meeting]
value = "~/.look/bin/meeting-next --tile"
"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        let meeting = tile(&resolved, "meeting").expect("the drawn tile");
        assert_eq!(meeting.role, look_qactions::TileRole::Custom);
        // The drawing's token is an id, so a label is made from it.
        assert_eq!(meeting.title, "Meeting");
        assert_eq!(cells(&resolved, "meeting"), Some((0, 0, 1, 1)));

        // The command stays off the wire: a shell renders a value the core
        // resolved and never learns what produced it.
        assert_eq!(
            resolved.defs["meeting"].value.as_deref(),
            Some("~/.look/bin/meeting-next --tile")
        );
    }

    #[test]
    fn a_hyphenated_name_is_title_cased_and_a_declared_title_wins() {
        let resolved = resolve(
            r#"
layout = ["ci-status  deploy"]

[tiles.ci-status]
value = "ci"

[tiles.deploy]
value = "deploy --status"
title = "Ship it"
"#,
        );
        assert_eq!(tile(&resolved, "ci-status").unwrap().title, "Ci Status");
        assert_eq!(tile(&resolved, "deploy").unwrap().title, "Ship it");
    }

    #[test]
    fn a_legend_entry_nobody_drew_is_inert_rather_than_an_error() {
        // How a user keeps a definition around while trying the grid without
        // it. Carrying it further would have the cache run a command for a tile
        // that is not on screen.
        let resolved = resolve(
            r#"
layout = ["mic"]

[tiles.meeting]
value = "meeting-next"
"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert!(resolved.defs.is_empty(), "nothing placed, nothing to run");
    }

    #[test]
    fn a_name_that_is_neither_built_in_nor_declared_says_both_things() {
        let resolved = resolve(r#"layout = ["mic  nosuchtile"]"#);
        assert_eq!(resolved.tiles.len(), 1);
        assert!(
            resolved.warnings[0].contains("[tiles.nosuchtile]"),
            "the message should name the entry that would fix it: {:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_tile_with_no_value_is_dropped_rather_than_shown_blank() {
        let resolved = resolve(
            r#"
layout = ["meeting  mic"]

[tiles.meeting]
refresh = "60s"
"#,
        );
        assert!(tile(&resolved, "meeting").is_none());
        assert!(tile(&resolved, "mic").is_some(), "the rest still renders");
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("neither a `value`")),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_reparse_is_skipped_until_the_file_changes() {
        /// Any distance the memo can tell apart; the value carries no meaning
        /// beyond being larger than every filesystem's timestamp granularity.
        const MTIME_STEP: Duration = Duration::from_secs(1);

        let _guard = ENV_GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let dir = std::env::temp_dir().join(format!("look-memo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("super-actions.toml");
        std::fs::write(&path, "layout = [\"mic\"]\n").expect("write");
        unsafe { std::env::set_var(LAUNCHPAD_FILE_ENV, &path) };

        assert_eq!(layout().tiles.len(), 1);

        // Same mtime: the memo answers and the new contents are not seen.
        let stamp = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .expect("mtime");
        std::fs::write(&path, "layout = [\"mic  theme\"]\n").expect("rewrite");
        // Exactly the original mtime, so the memo must treat it as unchanged.
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(stamp))
            .expect("rewind mtime");
        assert_eq!(
            layout().tiles.len(),
            1,
            "an unchanged mtime is not reparsed"
        );

        // A real edit moves the mtime, so the reload loop still works. Moved
        // explicitly, the way it was rewound: on Windows the file-time clock
        // advances in ~15ms steps and every write here lands within one, so a
        // rewrite alone can leave the mtime where the memo already saw it.
        std::fs::write(&path, "layout = [\"mic  theme\"]\n").expect("rewrite");
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(stamp + MTIME_STEP))
            .expect("advance mtime");
        assert_eq!(layout().tiles.len(), 2, "an edit is picked up");

        unsafe { std::env::remove_var(LAUNCHPAD_FILE_ENV) };
        *memo().lock().unwrap_or_else(|err| err.into_inner()) = None;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// LAUNCHPAD_FILE_ENV is process-global, so every test that points it
    /// somewhere takes this first. Without it two such tests interleave and one
    /// reads the developer's real super-actions.toml.
    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The only route a symbol has onto a button: it runs no command, so there
    /// is no JSON for one to arrive in.
    #[test]
    fn a_tile_can_name_its_own_symbol() {
        let resolved = resolve(
            r#"
layout = ["lock  mic"]

[tiles.lock]
press = "pmset displaysleepnow"
icon  = "lock.fill"
"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert_eq!(
            tile(&resolved, "lock")
                .expect("the drawn tile")
                .icon
                .as_deref(),
            Some("lock.fill")
        );
        assert_eq!(resolved.defs["lock"].icon.as_deref(), Some("lock.fill"));

        // A built-in's symbol stays with the shell that draws it.
        assert_eq!(tile(&resolved, "mic").expect("built-in").icon, None);
    }

    #[test]
    fn a_tile_that_only_acts_needs_nothing_to_display() {
        // Mic and Screensaver are built-ins of exactly this shape - an icon and
        // a name, nothing to report. A tile of your own that locks the screen
        // has no more to say than they do, and requiring a `value` would have
        // meant spawning a command every refresh just to print a constant.
        let resolved = resolve(
            r#"
layout = ["lock"]

[tiles.lock]
press = "pmset displaysleepnow"
"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);

        let lock = tile(&resolved, "lock").expect("the tile renders");
        assert!(lock.pressable);
        assert!(!lock.has_value, "so a shell draws a button, not a readout");
        assert!(
            resolved.defs["lock"].value.is_none(),
            "and nothing is ever run for it until it is pressed"
        );
    }

    #[test]
    fn a_tile_declares_what_it_runs_and_what_to_ask_first() {
        // Everything in one file. This used to be `block = "..."` naming an
        // entry in ~/.look/sources/, which put a tile's behaviour back across
        // two files and invented a way to fail - a block that is not installed
        // renders a tile that does nothing when pressed.
        let resolved = resolve(
            r#"
layout = ["deploy"]

[tiles.deploy]
value   = "deploy --status"
press   = "deploy --run"
confirm = "Deploy to production?"
"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        let def = &resolved.defs["deploy"];
        assert_eq!(def.press.as_deref(), Some("deploy --run"));
        assert_eq!(def.confirm.as_deref(), Some("Deploy to production?"));
    }

    #[test]
    fn a_confirm_with_nothing_to_confirm_is_pointed_out() {
        let resolved = resolve(
            r#"
layout = ["info"]

[tiles.info]
value   = "uptime"
confirm = "Really?"
"#,
        );
        // Still shown: it has a value, and that is what a tile is for.
        assert!(tile(&resolved, "info").is_some());
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("nothing asks it")),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_tile_value_carries_the_same_anatomy_a_built_in_shows() {
        // Weather shows a temperature, a condition, a high/low and a humidity.
        // A user tile beside it needs the same shape or it reads as a lesser
        // thing on the same screen.
        let parsed: TileValue = serde_json::from_str(
            r#"{"value":"29°","caption":"PARTLY CLOUDY","lines":["H 32° L 23°","41%"],
                "icon":"cloud.sun","state":"on"}"#,
        )
        .expect("the documented shape parses");

        assert_eq!(parsed.value, "29°");
        assert_eq!(parsed.caption.as_deref(), Some("PARTLY CLOUDY"));
        assert_eq!(parsed.lines, vec!["H 32° L 23°", "41%"]);
        assert_eq!(parsed.icon.as_deref(), Some("cloud.sun"));
        assert_eq!(parsed.state.as_deref(), Some("on"));
    }

    #[test]
    fn a_value_may_be_nothing_but_the_headline() {
        // The common case: one number from a shell one-liner. Every other field
        // is optional, so a script that only knows its value stays valid when
        // a field is added later.
        let parsed: TileValue = serde_json::from_str(r#"{"value":"3 failing"}"#).expect("parses");
        assert_eq!(parsed.value, "3 failing");
        assert!(parsed.caption.is_none());
        assert!(parsed.lines.is_empty());
    }

    #[test]
    fn refresh_reads_seconds_minutes_and_hours() {
        let resolved = resolve(
            r#"
layout = ["a  b  c  d"]

[tiles.a]
value = "x"
refresh = "90s"

[tiles.b]
value = "x"
refresh = "5m"

[tiles.c]
value = "x"
refresh = "2h"

[tiles.d]
value = "x"
refresh = "soon"
"#,
        );
        assert_eq!(resolved.defs["a"].refresh, Some(Duration::from_secs(90)));
        assert_eq!(resolved.defs["b"].refresh, Some(Duration::from_secs(300)));
        assert_eq!(resolved.defs["c"].refresh, Some(Duration::from_secs(7200)));

        // Unreadable is not fatal - the tile still shows, it just has no
        // staleness of its own - but it is said out loud.
        assert_eq!(resolved.defs["d"].refresh, None);
        assert!(
            resolved.warnings.iter().any(|w| w.contains("\"soon\"")),
            "{:?}",
            resolved.warnings
        );
    }

    // --- mnemonics ----------------------------------------------------------

    #[test]
    fn an_unclaimed_letter_is_granted() {
        let resolved = resolve(
            r#"
layout = ["ci"]

[tiles.ci]
value = "ci"
mnemonic = "c"
"#,
        );
        // Stored uppercase, matched case-insensitively by both shells.
        assert_eq!(tile(&resolved, "ci").unwrap().mnemonic, Some('C'));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
    }

    #[test]
    fn a_letter_a_built_in_already_answers_to_is_never_given_away() {
        // A config file must not be able to repoint Cmd+D away from Shut Down.
        let resolved = resolve(
            r#"
layout = ["deploy  shutdown"]

[tiles.deploy]
value = "deploy"
mnemonic = "D"
"#,
        );
        let deploy = tile(&resolved, "deploy").expect("the tile still renders");
        assert_eq!(deploy.mnemonic, None, "the key was refused");
        assert_eq!(
            tile(&resolved, "shutdown").unwrap().mnemonic,
            Some('D'),
            "and Shut Down kept it"
        );
        assert!(
            resolved.warnings.iter().any(|w| w.contains("shutdown")),
            "the warning should name who holds it: {:?}",
            resolved.warnings
        );
    }

    #[test]
    fn q_is_refused_because_the_os_takes_it_first() {
        // Cmd+Q is a menu shortcut, dispatched before Look's own key monitor,
        // so a tile holding it would silently never fire.
        let resolved = resolve(
            r#"
layout = ["quit"]

[tiles.quit]
value = "echo"
mnemonic = "Q"
"#,
        );
        assert_eq!(tile(&resolved, "quit").unwrap().mnemonic, None);
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("quitting Look")),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn between_two_user_tiles_the_first_drawn_keeps_the_key() {
        // The drawing fixes the order, so which one keeps it is answerable by
        // looking at the file rather than by knowing the map's iteration order.
        let resolved = resolve(
            r#"
layout = ["north  nudge"]

[tiles.north]
value = "x"
mnemonic = "N"

[tiles.nudge]
value = "x"
mnemonic = "N"
"#,
        );
        assert_eq!(tile(&resolved, "north").unwrap().mnemonic, Some('N'));
        assert_eq!(tile(&resolved, "nudge").unwrap().mnemonic, None);
        assert!(
            resolved.warnings.iter().any(|w| w.contains("\"north\"")),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_mnemonic_the_title_does_not_contain_is_flagged_but_kept() {
        // It fires, but neither shell can highlight it, so nobody would ever
        // discover it. Worth saying; not worth refusing.
        let resolved = resolve(
            r#"
layout = ["deploy"]

[tiles.deploy]
value = "x"
mnemonic = "X"
"#,
        );
        assert_eq!(tile(&resolved, "deploy").unwrap().mnemonic, Some('X'));
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("not shown on the tile")),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_two_letter_mnemonic_is_a_mistake_worth_naming() {
        let resolved = resolve(
            r#"
layout = ["ci"]

[tiles.ci]
value = "x"
mnemonic = "Cmd+C"
"#,
        );
        assert_eq!(tile(&resolved, "ci").unwrap().mnemonic, None);
        assert!(
            resolved
                .warnings
                .iter()
                .any(|w| w.contains("single character")),
            "{:?}",
            resolved.warnings
        );
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
    fn a_trailing_hole_is_a_column_the_shape_still_declares() {
        let resolved = resolve(r#"layout = ["mic . ."]"#);

        assert_eq!(resolved.columns, 3);
        assert_eq!(resolved.tiles.len(), 1);
        // How far the tiles reach, which is what the shells drew before the
        // shape travelled with them: one column, mic across the whole strip.
        let tile = &resolved.tiles[0];
        assert_eq!(tile.col + tile.col_span, 1);
    }

    #[test]
    fn more_columns_than_the_cap_are_dropped_rather_than_refused() {
        // Spelled out rather than derived from MAX_COLUMNS: a test that builds
        // its input and its expectation from the constant passes whatever the
        // constant says, which is the one thing worth catching here.
        let resolved = resolve(r#"layout = ["mic . . . . . ."]"#);

        assert_eq!(resolved.columns, 6);
        assert!(resolved.warnings[0].contains("first 6"));
        // Still a working launchpad, not a fallback, exactly as with the rows.
        assert!(!is_default(&resolved));
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
    fn a_tile_drawn_under_its_minimum_is_dropped_and_the_rest_renders() {
        // The shells dress by role, so a 1x1 nowplaying is a track title and
        // three transport buttons clipped into one cell.
        let resolved = resolve(
            r#"layout = [
                "nowplaying  theme  wifi",
            ]"#,
        );
        assert!(cells(&resolved, "nowplaying").is_none());
        assert_eq!(resolved.tiles.len(), 2);
        assert!(
            resolved.warnings[0].contains("needs at least 2x1 cells but is drawn 1x1"),
            "{:?}",
            resolved.warnings
        );
    }

    #[test]
    fn a_tile_drawn_at_exactly_its_minimum_is_kept() {
        // The boundary, in both axes.
        let resolved = resolve(
            r#"layout = [
                "lslot  lslot  weather",
                "lslot  lslot  weather",
            ]"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert_eq!(cells(&resolved, "lslot"), Some((0, 0, 2, 2)));
        assert_eq!(cells(&resolved, "weather"), Some((2, 0, 1, 2)));
    }

    #[test]
    fn a_tile_drawn_over_its_minimum_is_kept() {
        // A floor, not a size.
        let resolved = resolve(
            r#"layout = [
                "nowplaying  nowplaying  nowplaying",
                "nowplaying  nowplaying  nowplaying",
            ]"#,
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert_eq!(cells(&resolved, "nowplaying"), Some((0, 0, 3, 2)));
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

    /// A tile's legend line, and where its two right-hand columns sit: the key
    /// that fires it, then the smallest area it can be drawn in.
    fn legend(action_id: &str) -> (&'static str, (usize, &'static str), (usize, &'static str)) {
        let line = DEFAULT_CONTENTS
            .lines()
            .find(|line| line.starts_with(&format!("#   {action_id} ")))
            .unwrap_or_else(|| panic!("{action_id} has no legend line"));

        let min = line.split_whitespace().last().expect("a minimum column");
        let min_col = line.rfind(min).expect("the minimum is on the line");
        let head = line[..min_col].trim_end();
        let key = if head.ends_with("no key") {
            "no key"
        } else {
            &head[head.len() - 1..]
        };
        (line, (head.len() - key.len(), key), (min_col, min))
    }

    #[test]
    fn every_mnemonic_the_comment_claims_is_the_one_the_core_binds() {
        // The legend is documentation, and documentation that drifts is worse
        // than none: a user reads "D" and presses it expecting Shut Down.
        for tile in look_qactions::launchpad_layout() {
            let (line, (_, key), _) = legend(&tile.action_id);
            let expected = tile
                .mnemonic
                .map_or("no key".to_string(), |k| k.to_string());
            assert_eq!(
                key, expected,
                "the legend for {} is wrong: {line}",
                tile.action_id
            );
        }
    }

    #[test]
    fn every_minimum_the_comment_claims_is_the_one_the_core_enforces() {
        // Worse drift than the key: a user reads 1x1, draws it, and the tile
        // they drew is the one that does not appear.
        for tile in look_qactions::launchpad_layout() {
            let (line, _, (_, min)) = legend(&tile.action_id);
            let (col, row) = look_qactions::min_span(tile.role);
            assert_eq!(
                min,
                format!("{col}x{row}"),
                "the legend for {} is wrong: {line}",
                tile.action_id
            );
        }
    }

    #[test]
    fn the_legend_columns_line_up() {
        // This file is the documentation, and it is read in a text editor. A
        // key one column out is invisible to a test that only checks the field
        // itself, and obvious to everyone who opens the file.
        let columns: Vec<(usize, usize)> = look_qactions::launchpad_layout()
            .iter()
            .map(|tile| {
                let (_, (key, _), (min, _)) = legend(&tile.action_id);
                (key, min)
            })
            .collect();

        assert_eq!(columns.len(), 12, "one legend line per tile");
        assert!(
            columns.iter().all(|pair| *pair == columns[0]),
            "the legend columns are ragged: {columns:?}"
        );
    }

    /// A file the user may still be editing, so its presence is worth a line.
    #[test]
    fn the_file_this_one_replaced_is_pointed_out_while_it_still_exists() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = std::env::temp_dir().join(format!("look-legacy-{}", std::process::id()));
        let legacy = home.join(LEGACY_FILE_NAME);
        std::fs::create_dir_all(legacy.parent().expect("the .look directory")).expect("temp home");

        unsafe { std::env::remove_var(LAUNCHPAD_FILE_ENV) };
        assert_eq!(
            legacy_file_warning(&home),
            None,
            "no old file, nothing to say"
        );

        std::fs::write(&legacy, "layout = [\"mic\"]").expect("legacy file");
        let warning = legacy_file_warning(&home).expect("the old file is there");
        assert!(warning.contains(LEGACY_FILE_NAME), "{warning}");
        assert!(warning.contains(LAUNCHPAD_FILE_NAME), "{warning}");

        // A caller that named its own file has already answered the question.
        unsafe { std::env::set_var(LAUNCHPAD_FILE_ENV, home.join("elsewhere.toml")) };
        assert_eq!(legacy_file_warning(&home), None, "an override settles it");
        unsafe { std::env::remove_var(LAUNCHPAD_FILE_ENV) };

        let _ = std::fs::remove_dir_all(&home);
    }

    /// Both halves in one test on purpose: `LAUNCHPAD_FILE_ENV` is process
    /// global, and two tests setting it would race under the parallel runner.
    #[test]
    fn seeding_writes_the_default_once_and_never_touches_it_again() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let dir = std::env::temp_dir().join(format!("look-launchpad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("super-actions.toml");
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

    // --- writing the drawing back -------------------------------------------

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("look-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn placement(name: &str, col: u8, row: u8, col_span: u8, row_span: u8) -> Placement {
        Placement {
            action_id: name.to_string(),
            col,
            row,
            col_span,
            row_span,
        }
    }

    /// The rows the seed ships, read the way the resolver reads them.
    fn seeded_rows() -> Vec<String> {
        let table: toml::Value = toml::from_str(DEFAULT_CONTENTS).expect("the seed parses");
        table["layout"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|row| row.as_str().expect("a string").to_string())
            .collect()
    }

    /// The property the writer rests on: the default arrangement writes back
    /// as the seed, byte for byte, so a drag that ends where it began changes
    /// nothing in the file.
    #[test]
    fn the_default_layout_draws_as_the_seeded_file() {
        let tiles = look_qactions::launchpad_layout();
        let placements: Vec<Placement> = tiles.iter().map(Placement::from).collect();
        let (columns, rows) = extent(&tiles);

        assert_eq!(
            draw(&placements, columns, rows).expect("draws"),
            seeded_rows()
        );
    }

    #[test]
    fn a_hole_is_drawn_as_a_dot_and_a_trailing_track_survives() {
        // Holes take the same column width as names, so a dot lines up under
        // the name above it in a taller drawing.
        let drawn = draw(&[placement("mic", 0, 0, 1, 1)], 3, 1).expect("draws");
        assert_eq!(drawn, vec!["mic .   .".to_string()]);
    }

    #[test]
    fn two_tiles_on_one_cell_are_refused() {
        let err = draw(
            &[placement("mic", 0, 0, 1, 1), placement("wifi", 0, 0, 1, 1)],
            2,
            1,
        )
        .expect_err("refused");
        assert!(err.contains("both claim"), "{err}");
    }

    #[test]
    fn a_tile_past_the_edge_is_refused() {
        let err = draw(&[placement("nowplaying", 5, 0, 2, 1)], 6, 1).expect_err("refused");
        assert!(err.contains("past"), "{err}");
    }

    #[test]
    fn a_rewrite_keeps_every_comment_and_legend_entry() {
        let theirs = "# my strip\n\nlayout = [\n    \"mic   wifi\",\n]\n\n# what it runs\n[tiles.disk]\nvalue = \"echo hi\"   # trailing\n";

        let next = with_drawing(theirs, &["wifi mic".to_string()]).expect("rewrites");

        for kept in ["# my strip", "# what it runs", "[tiles.disk]", "# trailing"] {
            assert!(next.contains(kept), "{kept:?} was lost:\n{next}");
        }
        assert!(
            next.contains("layout = [\n    \"wifi mic\",\n]"),
            "the drawing is written the way the seed is:\n{next}"
        );
        assert!(
            !next.contains("mic   wifi"),
            "the old drawing is gone:\n{next}"
        );

        let resolved = resolve(&next);
        assert_eq!(cells(&resolved, "wifi"), Some((0, 0, 1, 1)));
        assert_eq!(cells(&resolved, "mic"), Some((1, 0, 1, 1)));
    }

    #[test]
    fn a_file_that_is_not_toml_is_left_alone() {
        let err = with_drawing("layout = [", &["mic".to_string()]).expect_err("refused");
        assert!(err.contains("not valid TOML"), "{err}");
    }

    #[test]
    fn a_file_without_a_drawing_gets_one_ahead_of_its_tables() {
        let next = with_drawing("[tiles.disk]\nvalue = \"echo hi\"\n", &["disk".to_string()])
            .expect("rewrites");

        let layout_at = next.find("layout = [").expect("a drawing");
        let table_at = next.find("[tiles.disk]").expect("the table");
        assert!(layout_at < table_at, "root keys come first:\n{next}");
        assert_eq!(cells(&resolve(&next), "disk"), Some((0, 0, 1, 1)));
    }

    #[test]
    fn a_save_seeds_a_missing_file_and_the_next_read_sees_the_move() {
        let dir = scratch_dir("save");
        let path = dir.join("super-actions.toml");

        // Mic and Restart trade places on the bottom row.
        let mut placements: Vec<Placement> = resolve(DEFAULT_CONTENTS)
            .tiles
            .iter()
            .map(Placement::from)
            .collect();
        for placement in &mut placements {
            match placement.action_id.as_str() {
                "mic" => placement.col = 1,
                "restart" => placement.col = 0,
                _ => {}
            }
        }

        save_layout_at(&path, &placements, 6, 3).expect("saves");

        let written = std::fs::read_to_string(&path).expect("written");
        assert!(
            written.contains("# Your Super Actions strip"),
            "seeded from the default, comments and all:\n{written}"
        );
        let resolved = resolve(&written);
        assert_eq!(cells(&resolved, "restart"), Some((0, 2, 1, 1)));
        assert_eq!(cells(&resolved, "mic"), Some((1, 2, 1, 1)));
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert!(
            !path.with_extension("toml.tmp").exists(),
            "nothing half-written is left behind"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_that_would_lose_a_tile_is_refused_before_the_file_is_touched() {
        let dir = scratch_dir("refuse");
        let path = dir.join("super-actions.toml");

        // Weather drawn 1x1, under its 1x2 floor: the resolver would drop it.
        let err =
            save_layout_at(&path, &[placement("weather", 0, 0, 1, 1)], 1, 1).expect_err("refused");

        assert!(err.contains("weather") && err.contains("at least"), "{err}");
        assert!(!path.exists(), "nothing was written");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A tile the file draws but the strip does not show is not on screen to
    /// be dragged, so writing the screen would erase it. The user has already
    /// been told the tile is wrong; this keeps it there for them to fix.
    #[test]
    fn a_save_that_would_erase_a_name_the_strip_does_not_show_is_refused() {
        let dir = scratch_dir("erase");
        let path = dir.join("super-actions.toml");
        // Weather 1x1 is under its floor: resolved as mic alone, with a warning.
        let theirs = "layout = [\"mic weather\"]\n";
        std::fs::write(&path, theirs).expect("write");
        assert_eq!(resolve(theirs).tiles.len(), 1, "weather is dropped");

        let err =
            save_layout_at(&path, &[placement("mic", 1, 0, 1, 1)], 2, 1).expect_err("refused");

        assert!(
            err.contains("\"weather\"") && err.contains("not on the strip"),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("still there"),
            theirs,
            "the file is untouched"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
