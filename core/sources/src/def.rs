//! The declared shape of a source: what it is, how it refreshes, what its rows
//! can do. Parsing only. Nothing here reads a directory or runs a command.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;

/// Immediate children only. Deep walks belong to the file index, not to a
/// source the user declared to get one short list.
pub const DEFAULT_FOLDER_DEPTH: usize = 1;

/// The action Enter runs. Any other key is an entry in the actions panel.
pub const DEFAULT_ACTION_KEY: &str = "default";

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;

const REFRESH_STARTUP: &str = "startup";
const REFRESH_OPEN: &str = "open";
const REFRESH_MANUAL: &str = "manual";

/// Which entries a folder source keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Only {
    Dirs,
    Files,
    #[default]
    All,
}

/// Whether the rows join the main result list or only their own scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Global,
    Prefix,
}

/// How the rows a source emits are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowFormat {
    #[default]
    Lines,
    Json,
}

/// Where an action runs. A launcher cannot host an interactive process, so
/// anything that needs a TTY is handed to the user's terminal instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    /// Detached, no window; the outcome is a banner.
    #[default]
    Background,
    /// Spawned in the user's terminal emulator.
    Terminal,
    /// Run, then show stdout in the panel.
    Output,
    /// Stdout becomes the next level of rows.
    Push,
}

/// When a command source re-runs. Never per keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Refresh {
    #[default]
    Startup,
    Open,
    Manual,
    Interval(Duration),
}

impl Refresh {
    /// `startup` | `open` | `manual` | a duration such as `30s`, `5m`, `1h`, `2d`.
    pub fn parse(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        match trimmed {
            REFRESH_STARTUP => return Ok(Self::Startup),
            REFRESH_OPEN => return Ok(Self::Open),
            REFRESH_MANUAL => return Ok(Self::Manual),
            _ => {}
        }

        let (digits, unit) = trimmed.split_at(
            trimmed
                .find(|c: char| !c.is_ascii_digit())
                .ok_or_else(|| format!("refresh \"{trimmed}\" has no unit, try \"5m\""))?,
        );
        let amount: u64 = digits
            .parse()
            .map_err(|_| format!("refresh \"{trimmed}\" is not a duration"))?;
        let seconds = match unit {
            "s" => amount,
            "m" => amount * SECONDS_PER_MINUTE,
            "h" => amount * SECONDS_PER_HOUR,
            "d" => amount * SECONDS_PER_DAY,
            other => return Err(format!("refresh unit \"{other}\" is not one of s, m, h, d")),
        };
        if seconds == 0 {
            return Err("refresh interval must be greater than zero".into());
        }
        Ok(Self::Interval(Duration::from_secs(seconds)))
    }
}

/// One thing the user can do with a selected row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub key: String,
    pub label: String,
    pub run: String,
    pub mode: ActionMode,
    /// Prompt for a typed value, substituted as `{input}`.
    pub input: Option<String>,
    /// Yes or no gate before running.
    pub confirm: Option<String>,
    /// Actions on the rows a `push` action returned.
    pub actions: Vec<Action>,
}

/// The kind-specific half of a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSpec {
    Folder {
        /// One list can gather several places: projects live under `~/dev` and
        /// `~/work` and are still one thing to the user. Written as `root` for
        /// the single case or `roots` for many; both land here.
        roots: Vec<String>,
        depth: usize,
        only: Only,
        include: Vec<String>,
        exclude: Vec<String>,
    },
    List {
        file: String,
        format: RowFormat,
    },
    Command {
        command: String,
        cwd: Option<String>,
        refresh: Refresh,
        timeout: Option<Duration>,
        format: RowFormat,
    },
}

impl SourceSpec {
    pub fn kind_key(&self) -> &'static str {
        match self {
            Self::Folder { .. } => "folder",
            Self::List { .. } => "list",
            Self::Command { .. } => "command",
        }
    }
}

/// A validated source. `id` is the file stem, which is what row ids and scoped
/// pruning key on, so it is never read from the file's own contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDef {
    pub id: String,
    pub name: String,
    pub spec: SourceSpec,
    pub scope: Scope,
    pub prefix: Option<String>,
    pub aliases: Vec<String>,
    pub bias: i64,
    pub icon: Option<String>,
    pub subtitle: Option<String>,
    pub enabled: bool,
    pub preview: Option<String>,
    pub actions: Vec<Action>,
    /// Keys the parser did not recognize. Reported, never fatal, so a file
    /// written for a newer version still loads.
    pub unknown_keys: Vec<String>,
}

impl SourceDef {
    pub fn action(&self, key: &str) -> Option<&Action> {
        self.actions.iter().find(|action| action.key == key)
    }

    pub fn default_action(&self) -> Option<&Action> {
        self.action(DEFAULT_ACTION_KEY)
    }
}

#[derive(Debug, Deserialize)]
struct RawAction {
    label: Option<String>,
    run: Option<String>,
    mode: Option<ActionMode>,
    input: Option<String>,
    confirm: Option<String>,
    #[serde(default)]
    actions: BTreeMap<String, RawAction>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSource {
    name: Option<String>,
    kind: Option<String>,
    scope: Option<Scope>,
    prefix: Option<String>,
    aliases: Option<Vec<String>>,
    bias: Option<i64>,
    icon: Option<String>,
    subtitle: Option<String>,
    enabled: Option<bool>,
    preview: Option<String>,

    root: Option<String>,
    roots: Option<Vec<String>>,
    depth: Option<usize>,
    only: Option<Only>,
    #[serde(rename = "match")]
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,

    file: Option<String>,

    command: Option<String>,
    cwd: Option<String>,
    refresh: Option<String>,
    timeout: Option<String>,
    format: Option<RowFormat>,

    #[serde(default)]
    actions: BTreeMap<String, RawAction>,
    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}

/// Parse a declared source. `id` is the caller's (the file stem);
/// `inferred_command` is the sibling or self executable used when the file
/// names no command of its own.
pub fn parse(
    id: &str,
    contents: &str,
    inferred_command: Option<&str>,
) -> Result<SourceDef, String> {
    let raw: RawSource = toml::from_str(contents).map_err(|err| err.to_string())?;
    build(id, raw, inferred_command)
}

/// A source with no declaration file at all: an executable dropped in the
/// directory, where everything comes from defaults.
pub fn inferred(id: &str, command: &str) -> SourceDef {
    build(id, RawSource::default(), Some(command))
        .expect("an inferred command source has every required field")
}

fn build(id: &str, raw: RawSource, inferred_command: Option<&str>) -> Result<SourceDef, String> {
    let mut unknown_keys: Vec<String> = raw.extra.keys().cloned().collect();

    let kind = match raw.kind.as_deref() {
        Some(value) => value.to_string(),
        None if raw.root.is_some() || raw.roots.is_some() => "folder".into(),
        None if raw.file.is_some() => "list".into(),
        None if raw.command.is_some() || inferred_command.is_some() => "command".into(),
        None => return Err("kind is missing and cannot be inferred".into()),
    };

    let spec = match kind.as_str() {
        "folder" => SourceSpec::Folder {
            roots: folder_roots(raw.root, raw.roots)?,
            depth: raw.depth.unwrap_or(DEFAULT_FOLDER_DEPTH),
            only: raw.only.unwrap_or_default(),
            include: raw.include.unwrap_or_default(),
            exclude: raw.exclude.unwrap_or_default(),
        },
        "list" => SourceSpec::List {
            file: raw.file.ok_or("list source needs a file")?,
            format: raw.format.unwrap_or_default(),
        },
        "command" => SourceSpec::Command {
            command: raw
                .command
                .or_else(|| inferred_command.map(String::from))
                .ok_or("command source needs a command")?,
            cwd: raw.cwd,
            refresh: raw
                .refresh
                .as_deref()
                .map(Refresh::parse)
                .transpose()?
                .unwrap_or_default(),
            timeout: raw
                .timeout
                .as_deref()
                .map(|value| match Refresh::parse(value)? {
                    Refresh::Interval(duration) => Ok(duration),
                    _ => Err(format!("timeout \"{value}\" must be a duration")),
                })
                .transpose()?,
            format: raw.format.unwrap_or_default(),
        },
        other => return Err(format!("kind \"{other}\" is not folder, list, or command")),
    };

    let scope = raw.scope.unwrap_or_default();
    if scope == Scope::Prefix && raw.prefix.is_none() {
        return Err("scope = \"prefix\" needs a prefix".into());
    }

    let mut actions = Vec::new();
    for (key, raw_action) in raw.actions {
        actions.push(build_action(&key, raw_action, &mut unknown_keys)?);
    }
    // The panel and Enter both read this list, so `default` leads and the rest
    // stay in a stable order rather than TOML's.
    actions.sort_by(|a, b| {
        let rank = |action: &Action| u8::from(action.key != DEFAULT_ACTION_KEY);
        rank(a).cmp(&rank(b)).then_with(|| a.key.cmp(&b.key))
    });

    Ok(SourceDef {
        id: id.to_string(),
        name: raw.name.unwrap_or_else(|| id.to_string()),
        spec,
        scope,
        prefix: raw.prefix,
        aliases: raw.aliases.unwrap_or_default(),
        bias: raw.bias.unwrap_or_default(),
        icon: raw.icon,
        subtitle: raw.subtitle,
        enabled: raw.enabled.unwrap_or(true),
        preview: raw.preview,
        actions,
        unknown_keys,
    })
}

/// `root` and `roots` are the same key in two shapes, so a user who starts with
/// one place and later adds another never has to restructure the file.
fn folder_roots(root: Option<String>, roots: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    out.extend(root);
    out.extend(roots.unwrap_or_default());
    out.retain(|value| !value.trim().is_empty());
    if out.is_empty() {
        return Err("folder source needs a root (or roots)".into());
    }
    Ok(out)
}

fn build_action(
    key: &str,
    raw: RawAction,
    unknown_keys: &mut Vec<String>,
) -> Result<Action, String> {
    for extra in raw.extra.keys() {
        unknown_keys.push(format!("actions.{key}.{extra}"));
    }

    let mut nested = Vec::new();
    for (nested_key, nested_raw) in raw.actions {
        nested.push(build_action(&nested_key, nested_raw, unknown_keys)?);
    }

    Ok(Action {
        key: key.to_string(),
        label: raw.label.unwrap_or_else(|| key.to_string()),
        run: raw
            .run
            .ok_or_else(|| format!("action \"{key}\" needs a run"))?,
        mode: raw.mode.unwrap_or_default(),
        input: raw.input,
        confirm: raw.confirm,
        actions: nested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_source_takes_its_defaults() {
        let def = parse("projects", "kind = \"folder\"\nroot = \"~/dev\"\n", None).unwrap();
        assert_eq!(def.name, "projects");
        assert_eq!(def.scope, Scope::Global);
        assert!(def.enabled);
        match def.spec {
            SourceSpec::Folder { depth, only, .. } => {
                assert_eq!(depth, DEFAULT_FOLDER_DEPTH);
                assert_eq!(only, Only::All);
            }
            other => panic!("expected a folder spec, got {other:?}"),
        }
    }

    #[test]
    fn kind_is_inferred_from_the_key_that_is_present() {
        let folder = parse("a", "root = \"~/dev\"\n", None).unwrap();
        assert_eq!(folder.spec.kind_key(), "folder");
        let list = parse("b", "file = \"~/hosts.txt\"\n", None).unwrap();
        assert_eq!(list.spec.kind_key(), "list");
        let command = parse("c", "command = \"ls\"\n", None).unwrap();
        assert_eq!(command.spec.kind_key(), "command");
    }

    #[test]
    fn an_executable_alone_is_a_whole_source() {
        let def = inferred("projects", "/home/u/.look/sources/projects");
        assert_eq!(def.name, "projects");
        assert!(def.enabled);
        match def.spec {
            SourceSpec::Command {
                command, refresh, ..
            } => {
                assert_eq!(command, "/home/u/.look/sources/projects");
                assert_eq!(refresh, Refresh::Startup);
            }
            other => panic!("expected a command spec, got {other:?}"),
        }
    }

    #[test]
    fn actions_put_default_first_then_stay_stable() {
        let def = parse(
            "repos",
            r#"
root = "~/dev"

[actions.zeta]
run = "z {path}"

[actions.default]
label = "Open"
run = "code {path}"

[actions.alpha]
run = "a {path}"
"#,
            None,
        )
        .unwrap();
        let keys: Vec<&str> = def.actions.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, ["default", "alpha", "zeta"]);
        assert_eq!(def.default_action().unwrap().label, "Open");
        // An action with no label is still usable; the key names it.
        assert_eq!(def.action("alpha").unwrap().label, "alpha");
    }

    #[test]
    fn run_values_survive_pipes_and_quotes_verbatim() {
        // The reason this format is TOML: a run value is shell text, and any
        // separator we invented would need escaping rules for it.
        let def = parse(
            "hosts",
            r#"
command = "awk '/^Host /{print $2}' ~/.ssh/config | sort"

[actions.default]
run = "$TERMINAL -e ssh {title} # connect"
"#,
            None,
        )
        .unwrap();
        match &def.spec {
            SourceSpec::Command { command, .. } => {
                assert_eq!(command, "awk '/^Host /{print $2}' ~/.ssh/config | sort");
            }
            other => panic!("expected a command spec, got {other:?}"),
        }
        assert_eq!(
            def.default_action().unwrap().run,
            "$TERMINAL -e ssh {title} # connect"
        );
    }

    #[test]
    fn nested_actions_describe_a_pushed_level() {
        let def = parse(
            "repos",
            r#"
root = "~/dev"

[actions.branches]
mode = "push"
run = "git -C {path} branch"

[actions.branches.actions.default]
label = "Checkout"
run = "git checkout {id}"

[actions.branches.actions.new]
input = "New branch name"
run = "git checkout -b {input}"
"#,
            None,
        )
        .unwrap();
        let branches = def.action("branches").unwrap();
        assert_eq!(branches.mode, ActionMode::Push);
        let keys: Vec<&str> = branches.actions.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, ["default", "new"]);
        assert_eq!(
            branches.actions[1].input.as_deref(),
            Some("New branch name")
        );
    }

    #[test]
    fn unknown_keys_are_reported_but_never_fatal() {
        let def = parse(
            "projects",
            r#"
root = "~/dev"
future_key = "whatever"

[actions.default]
run = "code {path}"
also_new = true
"#,
            None,
        )
        .unwrap();
        assert_eq!(def.unknown_keys, ["future_key", "actions.default.also_new"]);
    }

    #[test]
    fn missing_required_fields_name_what_is_missing() {
        assert!(
            parse("a", "kind = \"folder\"\n", None)
                .unwrap_err()
                .contains("root")
        );
        assert!(
            parse("b", "kind = \"list\"\n", None)
                .unwrap_err()
                .contains("file")
        );
        assert!(
            parse("c", "root = \"~/dev\"\nscope = \"prefix\"\n", None)
                .unwrap_err()
                .contains("prefix")
        );
        assert!(
            parse(
                "d",
                "root = \"~/dev\"\n\n[actions.default]\nlabel = \"x\"\n",
                None
            )
            .unwrap_err()
            .contains("run")
        );
    }

    #[test]
    fn refresh_accepts_keywords_and_durations() {
        assert_eq!(Refresh::parse("startup").unwrap(), Refresh::Startup);
        assert_eq!(Refresh::parse("open").unwrap(), Refresh::Open);
        assert_eq!(Refresh::parse("manual").unwrap(), Refresh::Manual);
        assert_eq!(
            Refresh::parse("90s").unwrap(),
            Refresh::Interval(Duration::from_secs(90))
        );
        assert_eq!(
            Refresh::parse("2h").unwrap(),
            Refresh::Interval(Duration::from_secs(2 * SECONDS_PER_HOUR))
        );
        assert!(Refresh::parse("1w").is_err());
        assert!(Refresh::parse("0m").is_err());
        assert!(Refresh::parse("soon").is_err());
    }
}
