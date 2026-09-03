//! Performing a block's steps.
//!
//! Every step goes through the user's LOGIN shell. A launcher is started by the
//! window server, so it inherits a minimal environment: no `$EDITOR`, no
//! `$TERMINAL`, and a `PATH` without Homebrew, nvm, or anything a profile adds.
//! Without `-l`, a command that works when pasted into a terminal fails here and
//! the user has no way to see why.
//!
//! Steps are detached and not waited on: the launcher window closes immediately
//! after Enter, and an app it launched must outlive it.
//!
//! A step is shell text down to its punctuation - `&&`, `>`, and the quotes
//! `expand` wraps every placeholder in. Which shell reads it is the platform's
//! answer: a POSIX one on Unix, where a `$SHELL` speaking another language is
//! passed over for `/bin/sh`, and `cmd` on Windows, where `COMSPEC` names it.
//! Quoting follows the same split, so a placeholder is inert in the shell that
//! will actually read it.

use std::io::Read;
use std::process::{Command, Stdio};

use serde::Deserialize;

#[cfg(windows)]
use look_tools::cmd_quote as quote;
#[cfg(not(windows))]
use look_tools::shell_quote as quote;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Used when `$SHELL` is unset, which happens for processes the window server
/// starts without a user session environment, and when it names a shell that
/// cannot read a POSIX step.
#[cfg(unix)]
const FALLBACK_SHELL: &str = "/bin/sh";

/// `$SHELL` is honoured only when it is one of these. `fish` and `nu` take
/// `-lc` too, so the spawn succeeds and the script is then rejected whole.
#[cfg(unix)]
const POSIX_SHELLS: [&str; 9] = [
    "sh", "bash", "dash", "ash", "zsh", "ksh", "ksh93", "mksh", "yash",
];

/// What `COMSPEC` is expected to name, and what is used when it names anything
/// else: the shell whose quoting `expand` writes for.
#[cfg(windows)]
const COMMAND_SHELL: &str = "cmd.exe";

/// Where the shell lives, relative to `SystemRoot`. Joined rather than left to
/// a `PATH` search: a `cmd.exe` dropped in a writable directory earlier on
/// `PATH` would win one, and every step would then go through it.
#[cfg(windows)]
const SYSTEM_SHELL_DIR: &str = "System32";

/// Used when `SystemRoot` is unset, which a stripped environment can be.
#[cfg(windows)]
const SYSTEM_ROOT_FALLBACK: &str = r"C:\Windows";

/// No console for a step the launcher starts. A `do` block opening an app must
/// not flash a black window, and a captured command has nothing to show.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Its own group, so the launcher exiting never signals what it started. The
/// Windows half of `setsid`.
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Said instead of running anything, so the step reads as unsupported rather
/// than as a missing program.
#[cfg(not(any(unix, windows)))]
const NO_SHELL: &str = "steps are shell commands, which this platform has no shell for";

/// How often a captured command is checked for having finished. Short enough
/// that a fast command is not held up, long enough not to spin.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Only the first line of stderr is ever shown, so there is no reason to hold
/// a failing command's whole diagnostic output in memory.
const MAX_STDERR_BYTES: usize = 8 * 1024;

const DRAIN_CHUNK_BYTES: usize = 8 * 1024;

/// Environment the step can read to know which row it was performed on.
pub const ENV_ID: &str = "LOOK_ID";
pub const ENV_TITLE: &str = "LOOK_TITLE";
pub const ENV_PATH: &str = "LOOK_PATH";

/// What a step was performed with, so a caller can report it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub step: String,
    pub error: Option<String>,
}

/// Runs every step in order, detached. One failing step does not stop the rest:
/// a bundle that opens four apps should open the three that are installed.
pub fn perform(steps: &[String], row: Option<&RowContext>) -> Vec<StepOutcome> {
    steps
        .iter()
        .map(|step| {
            let expanded = match row {
                Some(row) => expand(step, row),
                None => step.clone(),
            };
            let error = spawn(&expanded, row).err();
            StepOutcome {
                step: expanded,
                error,
            }
        })
        .collect()
}

/// The row a step is being performed on: what its placeholders expand to, and
/// what it exports to the process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowContext {
    pub id: String,
    pub title: String,
    pub path: String,
    /// What the user had typed when they picked the row.
    pub query: String,
    /// Rows this one was drilled from, NEAREST first: `{parent.id}` is
    /// `parents[0]`, and each further `parent.` steps one out.
    pub parents: Vec<ParentRow>,
}

impl RowContext {
    /// Its own path, or the nearest ancestor that has one: a script or branch
    /// row has none while the project above it does, and that is where "run
    /// this" means. A command can say `{parent.path}`; a directory cannot.
    pub fn working_path(&self) -> &str {
        if !self.path.is_empty() {
            return &self.path;
        }
        self.parents
            .iter()
            .map(|parent| parent.path.as_str())
            .find(|path| !path.is_empty())
            .unwrap_or_default()
    }

    /// The FOLDER to run in, never the file itself: `current_dir` on a file
    /// fails the spawn with ENOTDIR, which reads as `/bin/zsh: Not a directory`
    /// as if the user's command were broken.
    pub fn working_dir(&self) -> String {
        parent_dir(self.working_path())
    }
}

/// An ancestor row, for `{parent.*}`. Flat rather than a nested `RowContext`,
/// since a placeholder can name nothing else about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ParentRow {
    pub id: String,
    pub title: String,
    pub path: String,
}

/// One list, so `expand` and `Block::needs_row` agree on what a placeholder is.
/// A brace that opens anything else is not one: a `run` command emitting JSON
/// is full of them.
pub const PLACEHOLDER_ID: &str = "{id}";
pub const PLACEHOLDER_TITLE: &str = "{title}";
pub const PLACEHOLDER_PATH: &str = "{path}";
pub const PLACEHOLDER_DIR: &str = "{dir}";
pub const PLACEHOLDER_QUERY: &str = "{query}";
/// Opens an ancestor placeholder (`{parent.id}`), which also means a producer
/// mentioning one needs a row.
pub const PLACEHOLDER_PARENT: &str = "{parent.";
pub const PLACEHOLDERS: [&str; 5] = [
    PLACEHOLDER_ID,
    PLACEHOLDER_TITLE,
    PLACEHOLDER_PATH,
    PLACEHOLDER_DIR,
    PLACEHOLDER_QUERY,
];

/// Substitutes `{id}`, `{title}`, `{path}`, `{dir}`, and `{query}`.
///
/// Every value is quoted on the way in, which is a correctness requirement
/// before it is a safety one: rows come from directory listings and command
/// output, so a folder called `my project` must stay one argument and a row
/// titled `; rm -rf ~` must not execute. A template therefore never quotes its
/// own placeholders.
pub fn expand(template: &str, row: &RowContext) -> String {
    substitute(template, row, quote)
}

/// The same placeholders, unquoted, for a value that becomes a filesystem path
/// rather than shell text: quotes there land in the path and nothing is found.
/// Hand the result to the filesystem, never to a shell.
pub fn expand_path(template: &str, row: &RowContext) -> String {
    substitute(template, row, |value| value.to_string())
}

/// One pass, so the shell and filesystem forms cannot drift. Ancestors go
/// deepest prefix first; a missing one reads as empty, like a row with no path.
fn substitute(template: &str, row: &RowContext, transform: fn(&str) -> String) -> String {
    let mut out = template.to_string();

    // As deep as the row goes OR as deep as the template asks: an ancestor the
    // row lacks must still be substituted, or the literal text reaches a shell.
    let deepest = deepest_parent_depth(template).max(row.parents.len()).max(1);
    for depth in (1..=deepest).rev() {
        let prefix = PLACEHOLDER_PARENT_WORD.repeat(depth);
        let parent = row.parents.get(depth - 1);
        let (id, title, path) = match parent {
            Some(parent) => (
                parent.id.as_str(),
                parent.title.as_str(),
                parent.path.as_str(),
            ),
            None => ("", "", ""),
        };
        out = out
            .replace(&format!("{{{prefix}id}}"), &transform(id))
            .replace(&format!("{{{prefix}title}}"), &transform(title))
            .replace(&format!("{{{prefix}path}}"), &transform(path))
            .replace(&format!("{{{prefix}dir}}"), &transform(&parent_dir(path)));
    }

    let dir = parent_dir(&row.path);
    out.replace(PLACEHOLDER_ID, &transform(&row.id))
        .replace(PLACEHOLDER_TITLE, &transform(&row.title))
        .replace(PLACEHOLDER_PATH, &transform(&row.path))
        .replace(PLACEHOLDER_DIR, &transform(&dir))
        .replace(PLACEHOLDER_QUERY, &transform(&row.query))
}

/// The word an ancestor placeholder repeats, without the brace.
const PLACEHOLDER_PARENT_WORD: &str = "parent.";

/// The deepest `{parent.parent....}` the template names. No ceiling: a depth
/// the search stops short of is a placeholder handed to a shell as text.
fn deepest_parent_depth(template: &str) -> usize {
    let mut deepest = 0;
    for (open, _) in template.match_indices(PLACEHOLDER_PARENT) {
        let mut rest = &template[open + 1..];
        let mut depth = 0;
        while let Some(shorter) = rest.strip_prefix(PLACEHOLDER_PARENT_WORD) {
            depth += 1;
            rest = shorter;
        }
        deepest = deepest.max(depth);
    }
    deepest
}

/// The row's own folder: itself when it is one, its parent otherwise.
fn parent_dir(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let as_path = std::path::Path::new(path);
    if as_path.is_dir() {
        return path.to_string();
    }
    as_path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The login shell to run a step with: the user's own where it speaks POSIX,
/// the system one otherwise. Set-but-empty reads as `Ok("")`, so it falls here
/// too.
#[cfg(unix)]
fn posix_shell(configured: &str) -> &str {
    let name = std::path::Path::new(configured)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if POSIX_SHELLS.contains(&name) {
        configured
    } else {
        FALLBACK_SHELL
    }
}

/// The one place a step becomes a process: `<shell> -lc <step>`, or nothing at
/// all where there is no POSIX shell to read it.
#[cfg(unix)]
fn shell_command(step: &str) -> Result<Command, String> {
    let configured = std::env::var("SHELL").unwrap_or_default();
    let mut command = Command::new(posix_shell(&configured));
    command
        .arg("-lc")
        .arg(step)
        // An AppImage runtime points this at its own bundled libs, and a host
        // binary resolving against them dies on a symbol lookup. A step is the
        // user's own command, so it must see the host's loader path, not ours.
        .env_remove("LD_LIBRARY_PATH");
    Ok(command)
}

/// `COMSPEC` when it names `cmd` by absolute path, which is the only thing it
/// names in practice, and the copy under `SystemRoot` otherwise: a step is
/// written in `cmd`'s language and quoted for it, so another interpreter would
/// read it wrong rather than better.
#[cfg(windows)]
fn command_shell() -> std::path::PathBuf {
    let configured = std::path::PathBuf::from(std::env::var_os("COMSPEC").unwrap_or_default());
    let names_cmd = configured.is_absolute()
        && configured
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem.eq_ignore_ascii_case("cmd"));
    if names_cmd {
        configured
    } else {
        system_shell()
    }
}

/// `%SystemRoot%\System32\cmd.exe`, resolved rather than searched for.
#[cfg(windows)]
fn system_shell() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot")
        .filter(|root| !root.is_empty())
        .unwrap_or_else(|| SYSTEM_ROOT_FALLBACK.into());
    std::path::Path::new(&root)
        .join(SYSTEM_SHELL_DIR)
        .join(COMMAND_SHELL)
}

/// The one place a step becomes a process on Windows: `cmd /D /S /C "<step>"`.
///
/// `/D` skips the AutoRun command the registry can hold, which a launcher must
/// not inherit into every step. `/S` makes the quoting rule the simple one -
/// strip the outer pair, take the rest verbatim - which is what lets a step keep
/// the quotes `expand` put around its placeholders.
#[cfg(windows)]
fn shell_command(step: &str) -> Result<Command, String> {
    use std::os::windows::process::CommandExt;

    let mut command = Command::new(command_shell());
    // `raw_arg`, never `arg`: std escapes for the MSVCRT argv rules, which `cmd`
    // does not read, so a step carrying a quote would arrive mangled.
    command
        .raw_arg(format!("/D /S /C \"{step}\""))
        .creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

#[cfg(not(any(unix, windows)))]
fn shell_command(_step: &str) -> Result<Command, String> {
    Err(NO_SHELL.to_string())
}

/// Names the shell that could not start: "no such file or directory" alone
/// names nothing the user can act on.
fn spawn_failure(command: &Command, err: std::io::Error) -> String {
    format!("{}: {err}", command.get_program().to_string_lossy())
}

fn spawn(step: &str, row: Option<&RowContext>) -> Result<(), String> {
    let mut command = shell_command(step)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(row) = row {
        command
            .env(ENV_ID, &row.id)
            .env(ENV_TITLE, &row.title)
            .env(ENV_PATH, &row.path);
        // The row's FOLDER, never the row itself: `path` is a file for most
        // rows, and `current_dir` on a file makes every spawn fail with
        // ENOTDIR. Same rule as `{dir}`.
        let dir = row.working_dir();
        if !dir.is_empty() {
            command.current_dir(&dir);
        }
    }

    detach(&mut command);
    match command.spawn() {
        Ok(_) => Ok(()),
        Err(err) => Err(spawn_failure(&command, err)),
    }
}

/// Runs `command` and returns its stdout, for the producers whose output IS the
/// answer (a `run` block's rows, a `preview`).
///
/// Bounded three ways, because this is arbitrary user code on a path the user is
/// waiting on: a timeout, a byte cap, and no stdin. Unlike `perform`, this waits
/// - the caller is asking for the output, so there is nothing to detach from.
pub fn capture(
    command: &str,
    cwd: Option<&str>,
    timeout: Duration,
    max_bytes: usize,
) -> Result<String, String> {
    let mut shell = shell_command(command)?;
    shell
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(
            cwd.filter(|dir| !dir.is_empty())
                .map(str::to_string)
                .unwrap_or_else(root_dir),
        );
    // Before the spawn, so the shell leads its own group from its first
    // instruction and the deadline below has a whole tree to aim at.
    lead_process_group(&mut shell);
    let mut spawned = shell.spawn().map_err(|err| spawn_failure(&shell, err))?;

    // Both pipes are drained on their own threads for the whole run. Polling
    // `try_wait` and reading afterwards deadlocks the moment a command writes
    // more than the OS pipe buffer (~64 KiB): the child blocks on write, never
    // exits, and the deadline then reports a timeout for a healthy command.
    let stdout = spawned.stdout.take().map(|pipe| drain(pipe, max_bytes));
    let stderr = spawned
        .stderr
        .take()
        .map(|pipe| drain(pipe, MAX_STDERR_BYTES));

    let deadline = Instant::now() + timeout;
    let status = loop {
        match spawned.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_tree(&mut spawned);
                    let _ = spawned.wait();
                    return Err(format!("timed out after {}s", timeout.as_secs()));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => return Err(err.to_string()),
        }
    };

    let out = stdout.map(join_drained).unwrap_or_default();
    if !status.success() {
        let errors = stderr.map(join_drained).unwrap_or_default();
        let reason = errors.lines().next().unwrap_or("exited non-zero").trim();
        return Err(if reason.is_empty() {
            "exited non-zero".to_string()
        } else {
            reason.to_string()
        });
    }
    Ok(out)
}

/// Where a captured command runs when its block named no `cwd`: a directory
/// that is certain to exist, since the launcher's own may have been deleted.
#[cfg(not(windows))]
fn root_dir() -> String {
    "/".to_string()
}

/// `\` alone is the current drive's root, which is not where the launcher was
/// started from. `SystemDrive` names the one that is always mounted.
#[cfg(windows)]
fn root_dir() -> String {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    format!("{drive}\\")
}

/// Reads a pipe to its end on its own thread, keeping at most `max_bytes`.
///
/// It keeps reading past the cap rather than stopping, because closing the pipe
/// early would hand the child a SIGPIPE and turn "your output was long" into
/// "your command failed".
fn drain<R: Read + Send + 'static>(mut pipe: R, max_bytes: usize) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut kept: Vec<u8> = Vec::new();
        let mut chunk = [0u8; DRAIN_CHUNK_BYTES];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 {
                break;
            }
            let room = max_bytes.saturating_sub(kept.len());
            if room > 0 {
                kept.extend_from_slice(&chunk[..read.min(room)]);
            }
        }
        kept
    })
}

/// The drained bytes as text, ending on a whole character.
///
/// Mid-stream invalid bytes still decode lossily, since genuinely non-UTF8
/// output is worth showing. Only a trailing replacement character is dropped:
/// there it is not the command's data but an artifact of the byte cap landing
/// inside a multi-byte glyph.
fn join_drained(handle: JoinHandle<Vec<u8>>) -> String {
    let bytes = handle.join().unwrap_or_default();
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    let cut_a_glyph = std::str::from_utf8(&bytes).is_err();
    if cut_a_glyph && text.ends_with(char::REPLACEMENT_CHARACTER) {
        text.pop();
    }
    text
}

/// Its own process group, so closing the launcher never signals what it started.
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            libc_setsid();
            Ok(())
        });
    }
}

#[cfg(unix)]
fn libc_setsid() {
    unsafe extern "C" {
        fn setsid() -> i32;
    }
    unsafe {
        setsid();
    }
}

/// The shell in a process group of its own, so a deadline can reach everything
/// it started and not just the shell.
///
/// The same `setsid` call `detach` makes, for the opposite reason: there it
/// keeps the launcher's exit from reaching a step, here it makes the step's
/// whole subtree reachable by one signal.
#[cfg(unix)]
fn lead_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    // Runs between fork and exec, where only async-signal-safe calls are legal.
    // `setsid` is one; nothing else belongs in this closure.
    unsafe {
        command.pre_exec(|| {
            libc_setsid();
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn lead_process_group(_command: &mut Command) {}

/// Kills the shell and everything it started.
///
/// `Child::kill` signals the shell alone. A step is usually a pipeline or a
/// chain, so the thing actually doing the work - `sqlite3`, a `curl`, a `brew` -
/// is a separate process that survives, orphaned and still holding whatever it
/// held. A `run` block fires once per reload, so one stray is a nuisance nobody
/// notices. Anything on a path the user waits on, or worse fires every time a
/// window opens, accumulates them for as long as the app is up.
#[cfg(unix)]
fn kill_tree(spawned: &mut std::process::Child) {
    unsafe extern "C" {
        fn getpgid(pid: i32) -> i32;
        fn killpg(pgrp: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;

    let group = unsafe { getpgid(spawned.id() as i32) };
    // Never signal our own group. If `setsid` did not take, the shell is still
    // in the launcher's group, and killpg would take the launcher down with it.
    if group > 0 && group != unsafe { getpgid(0) } {
        unsafe {
            killpg(group, SIGKILL);
        }
    }
    // Still kill the shell directly: the group call is skipped in exactly the
    // case where it would be unsafe, and this is what that case falls back to.
    let _ = spawned.kill();
}

/// Windows kills the shell only. Reaching the rest of the tree needs a job
/// object, which is a different piece of work from this one; `cmd` steps are
/// also less often pipelines, so the exposure is smaller.
#[cfg(not(unix))]
fn kill_tree(spawned: &mut std::process::Child) {
    let _ = spawned.kill();
}

/// Same intent as the Unix `setsid`, plus the console suppression `cmd` needs:
/// `creation_flags` replaces rather than adds, so both flags are set here.
#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn detach(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str) -> RowContext {
        RowContext {
            id: "look".into(),
            title: "look".into(),
            path: path.into(),
            query: "loo".into(),
            ..Default::default()
        }
    }

    /// The platform's quoting, so a test says which placeholder was quoted
    /// rather than repeating one shell's spelling of it. The spellings
    /// themselves are asserted by `look_tools`.
    fn q(value: &str) -> String {
        quote(value)
    }

    fn drilled(parents: &[(&str, &str)]) -> RowContext {
        RowContext {
            id: "build".into(),
            title: "build".into(),
            query: String::new(),
            path: String::new(),
            parents: parents
                .iter()
                .map(|(id, path)| ParentRow {
                    id: (*id).into(),
                    title: (*id).into(),
                    path: (*path).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_command_for_a_file_row_runs_in_the_files_folder_not_on_the_file() {
        // `current_dir` on a file is ENOTDIR: the shell never starts, and the
        // user sees "/bin/zsh: Not a directory" as if their command were wrong.
        let dir = std::env::temp_dir().join(format!("look-workdir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("probe dir");
        let file = dir.join("changed.txt");
        std::fs::write(&file, "x").expect("probe file");

        let row = RowContext {
            path: file.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(row.working_dir(), dir.to_string_lossy());

        // A row with no path of its own borrows the nearest ancestor's folder.
        let drilled = RowContext {
            parents: vec![ParentRow {
                path: dir.to_string_lossy().into_owned(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(drilled.working_dir(), dir.to_string_lossy());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_ancestor_is_named_by_stepping_out_one_level_at_a_time() {
        let row = drilled(&[("animate", "/dev/animate"), ("prod", "")]);
        assert_eq!(
            expand("npm --prefix {parent.path} run {id}", &row),
            format!("npm --prefix {} run {}", q("/dev/animate"), q("build"))
        );
        assert_eq!(
            expand(
                "k --context {parent.parent.id} -n {parent.id} logs {id}",
                &row
            ),
            format!(
                "k --context {} -n {} logs {}",
                q("prod"),
                q("animate"),
                q("build")
            )
        );
    }

    #[test]
    fn an_ancestor_deeper_than_the_chain_still_reads_as_empty() {
        let row = drilled(&[("animate", "/dev/animate")]);
        assert_eq!(
            expand(
                "k --context {parent.parent.id} -n {parent.id} logs {id}",
                &row
            ),
            format!(
                "k --context {} -n {} logs {}",
                q(""),
                q("animate"),
                q("build")
            )
        );
    }

    #[test]
    fn no_depth_is_deep_enough_to_leave_a_placeholder_behind() {
        let deep = "x {parent.parent.parent.parent.parent.parent.parent.parent.parent.id}";
        assert_eq!(
            expand(deep, &drilled(&[("one", "/one")])),
            format!("x {}", q(""))
        );
    }

    #[test]
    fn an_ancestor_that_is_not_there_reads_as_empty_rather_than_as_itself() {
        // A literal `{parent.path}` reaching the shell would be worse: the
        // command would run against a directory named after the placeholder.
        let row = drilled(&[]);
        assert_eq!(expand("ls {parent.path}", &row), format!("ls {}", q("")));
    }

    #[test]
    fn an_ancestor_placeholder_is_quoted_for_a_shell_and_raw_for_a_path() {
        let row = drilled(&[("my project", "/dev/my project")]);
        assert_eq!(
            expand("cd {parent.path}", &row),
            format!("cd {}", q("/dev/my project"))
        );
        assert_eq!(
            expand_path("{parent.path}/src", &row),
            "/dev/my project/src"
        );
    }

    #[test]
    fn placeholders_expand_to_the_selected_row() {
        let expanded = expand("nvim {path} # {title} {query}", &row("/tmp/look"));
        assert_eq!(
            expanded,
            format!("nvim {} # {} {}", q("/tmp/look"), q("look"), q("loo"))
        );
    }

    #[test]
    fn a_path_with_spaces_stays_one_argument() {
        assert_eq!(
            expand("code {path}", &row("/tmp/my project")),
            format!("code {}", q("/tmp/my project"))
        );
    }

    #[test]
    fn a_row_named_like_a_command_cannot_execute() {
        // Rows come from directory listings and command output, so this is the
        // difference between quoting and a deleted home directory.
        let mut hostile = row("");
        hostile.title = "; rm -rf ~".into();
        assert_eq!(
            expand("echo {title}", &hostile),
            format!("echo {}", q("; rm -rf ~"))
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn an_embedded_quote_survives_intact() {
        let mut tricky = row("");
        tricky.title = "it's".into();
        assert_eq!(expand("echo {title}", &tricky), "echo 'it'\\''s'");
    }

    /// The quote `cmd` cares about is the double one, and a row carrying it must
    /// not be able to close the argument it sits in.
    #[cfg(windows)]
    #[test]
    fn an_embedded_quote_survives_intact() {
        let mut tricky = row("");
        tricky.title = "say \"hi\" & whoami".into();
        assert_eq!(
            expand("echo {title}", &tricky),
            "echo \"say \"\"hi\"\" & whoami\""
        );
    }

    #[test]
    fn dir_is_the_row_when_it_is_a_folder_and_its_parent_otherwise() {
        let dir = std::env::temp_dir().join(format!("look-expand-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("probe dir");
        let dir_text = dir.to_str().expect("utf8 path");

        assert_eq!(expand("{dir}", &row(dir_text)), q(dir_text));

        let file = dir.join("probe.txt");
        std::fs::write(&file, "x").expect("probe file");
        assert_eq!(
            expand("{dir}", &row(file.to_str().unwrap())),
            q(dir_text),
            "a file row hands its command the folder it lives in"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_cannot_read_a_posix_step_is_not_the_one_used() {
        // What the user logs in with is kept wherever it can read the step:
        // that is where `$PATH` comes from.
        assert_eq!(
            posix_shell("/opt/homebrew/bin/bash"),
            "/opt/homebrew/bin/bash"
        );
        assert_eq!(posix_shell("/bin/zsh"), "/bin/zsh");
        assert_eq!(posix_shell("/usr/bin/fish"), FALLBACK_SHELL);
        assert_eq!(posix_shell("/usr/local/bin/nu"), FALLBACK_SHELL);
        assert_eq!(
            posix_shell(""),
            FALLBACK_SHELL,
            "set but empty is not a shell"
        );
    }

    /// Everything below performs a real command.
    #[cfg(unix)]
    mod shell {
        use super::*;

        #[test]
        fn capture_returns_what_the_command_printed() {
            let out = capture("printf 'a\nb\n'", None, Duration::from_secs(5), 1024).unwrap();
            assert_eq!(out, "a\nb\n");
        }

        #[test]
        fn a_command_that_fails_reports_its_first_stderr_line() {
            let err =
                capture("echo boom >&2; exit 1", None, Duration::from_secs(5), 1024).unwrap_err();
            assert_eq!(err, "boom");
        }

        #[test]
        fn a_hung_command_is_killed_rather_than_waited_on() {
            let err = capture("sleep 30", None, Duration::from_millis(150), 1024).unwrap_err();
            assert!(err.contains("timed out"), "{err}");
        }

        #[test]
        fn a_timeout_takes_what_the_step_started_with_it() {
            // A step is usually a pipeline or a chain, so the process doing the
            // work is not the shell we hold. Killing only the shell leaves that
            // one running, orphaned, for as long as it likes - and nothing ever
            // reports it, because from the outside the timeout looks handled.
            //
            // The step reports its own child's pid rather than the test matching
            // on a command line: `sleep 45 | grep <marker>` only puts the marker
            // in the grep's argv, so a check by name would watch the wrong half
            // of the pipeline and pass while the sleep leaked.
            // Wide on purpose: it only has to land between the step recording
            // its pid and `sleep 45` ending. A step runs under `-lc`, and a
            // login shell on a loaded runner does not always reach its first
            // command inside a couple of hundred milliseconds.
            const DEADLINE: Duration = Duration::from_secs(2);

            let pid_file = std::env::temp_dir().join(format!("look-orphan-{}", std::process::id()));
            let _ = std::fs::remove_file(&pid_file);

            let err = capture(
                &format!("sleep 45 & echo $! > {} ; wait", pid_file.display()),
                None,
                DEADLINE,
                1024,
            )
            .unwrap_err();
            assert!(err.contains("timed out"), "{err}");

            let child: i32 = std::fs::read_to_string(&pid_file)
                .unwrap_or_else(|err| {
                    panic!("the step never recorded a pid in {DEADLINE:?}: {err}")
                })
                .trim()
                .parse()
                .expect("a pid");
            let _ = std::fs::remove_file(&pid_file);

            // The kill and the reparenting are not instant.
            std::thread::sleep(Duration::from_millis(300));

            // `kill(pid, 0)` asks whether the process exists and signals
            // nothing. Not `ps -p`: on macOS that lists only processes on the
            // current terminal, and an orphan has none - so it reports every
            // leaked process as gone, which is precisely the answer that would
            // make this test pass while the bug was present.
            unsafe extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            let alive = unsafe { kill(child, 0) } == 0;

            assert!(
                !alive,
                "the process the step started ({child}) outlived the timeout"
            );
        }

        #[test]
        fn output_larger_than_the_pipe_buffer_does_not_deadlock() {
            // The OS pipe buffer is ~64 KiB. Reading only after the child exits
            // means it blocks on write and never exits, and the deadline then
            // reports a timeout for a command that was working fine.
            let out = capture(
                "head -c 200000 /dev/zero | tr '\\0' 'a'",
                None,
                Duration::from_secs(10),
                256 * 1024,
            )
            .expect("a long-output command must finish");
            assert_eq!(out.len(), 200_000);
        }

        #[test]
        fn output_past_the_cap_is_cut_on_a_char_boundary() {
            // A multi-byte glyph straddling the cap must not panic or corrupt.
            let out = capture(
                "printf 'é%.0s' $(seq 1 100)",
                None,
                Duration::from_secs(5),
                15,
            )
            .unwrap();
            assert!(out.len() <= 15);
            assert!(out.chars().all(|c| c == 'é'), "{out:?}");
        }

        #[test]
        fn a_step_on_a_file_row_runs_in_the_files_folder() {
            // `current_dir` on a file path fails the spawn outright, so this is the
            // difference between file rows working and every one of them erroring.
            let dir = std::env::temp_dir().join(format!("look-run-cwd-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("probe dir");
            let file = dir.join("row.txt");
            std::fs::write(&file, "x").expect("probe file");

            let mut context = row("");
            context.path = file.to_string_lossy().into_owned();
            let outcomes = perform(&["true".to_string()], Some(&context));

            assert_eq!(outcomes[0].error, None, "a file row must not fail to spawn");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn every_step_is_attempted_even_when_one_fails() {
            // A bundle that opens four apps should open the three that exist, so a
            // failure is recorded against its own step and the rest still run.
            let steps = vec!["true".to_string(), "false".to_string(), "true".to_string()];
            let outcomes = perform(&steps, None);
            assert_eq!(outcomes.len(), 3);
            // Spawning succeeds for all three: a nonzero exit is the command's
            // business, not a spawn failure, and we never wait for it.
            assert!(outcomes.iter().all(|outcome| outcome.error.is_none()));
        }

        #[test]
        fn a_step_runs_through_a_shell_so_shell_syntax_works() {
            let path = std::env::temp_dir().join(format!("look-run-{}", std::process::id()));
            let _ = std::fs::remove_file(&path);
            let steps = vec![format!("echo hi > {}", path.display())];

            let outcomes = perform(&steps, None);
            assert!(outcomes[0].error.is_none());

            // Detached, so waited for: a login shell reads the profile first,
            // which on a loaded machine is not instant.
            let deadline = Instant::now() + Duration::from_secs(10);
            while !path.exists() && Instant::now() < deadline {
                std::thread::sleep(POLL_INTERVAL);
            }
            assert!(
                path.exists(),
                "redirection is shell syntax, so it needs a shell"
            );
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Everything below performs a real command through `cmd`.
    #[cfg(windows)]
    mod cmd {
        use super::*;

        #[test]
        fn comspec_is_used_only_when_it_names_cmd() {
            // Steps are written in `cmd`'s language and quoted for it, so an
            // interpreter that reads them differently is not an improvement.
            let shell = command_shell();
            assert!(
                shell
                    .file_stem()
                    .is_some_and(|stem| stem.eq_ignore_ascii_case("cmd")),
                "{}",
                shell.display()
            );
            // Absolute either way: a bare name is a `PATH` search, and a step
            // must not go through whatever wins one.
            assert!(shell.is_absolute(), "{}", shell.display());
        }

        #[test]
        fn capture_returns_what_the_command_printed() {
            let out = capture("echo a", None, Duration::from_secs(5), 1024).unwrap();
            assert_eq!(out.trim_end(), "a");
        }

        #[test]
        fn a_command_that_fails_reports_its_first_stderr_line() {
            let err = capture(
                "echo boom 1>&2 & exit /b 1",
                None,
                Duration::from_secs(5),
                1024,
            )
            .unwrap_err();
            assert_eq!(err, "boom");
        }

        #[test]
        fn a_hung_command_is_killed_rather_than_waited_on() {
            let err = capture(
                "ping -n 30 127.0.0.1 >nul",
                None,
                Duration::from_millis(150),
                1024,
            )
            .unwrap_err();
            assert!(err.contains("timed out"), "{err}");
        }

        #[test]
        fn a_quoted_placeholder_is_text_and_not_a_second_command() {
            // Rows come from directory listings and command output, so this is
            // the difference between quoting and running what a row is named.
            let mut hostile = row("");
            hostile.title = "a & echo pwned".into();
            let out = capture(
                &expand("echo {title}", &hostile),
                None,
                Duration::from_secs(5),
                1024,
            )
            .unwrap();
            assert_eq!(out.trim_end(), "\"a & echo pwned\"");
        }

        #[test]
        fn a_step_runs_through_a_shell_so_shell_syntax_works() {
            let path = std::env::temp_dir().join(format!("look-run-{}", std::process::id()));
            let _ = std::fs::remove_file(&path);

            let outcomes = perform(&[format!("echo hi> \"{}\"", path.display())], None);
            assert!(outcomes[0].error.is_none(), "{:?}", outcomes[0].error);

            let deadline = Instant::now() + Duration::from_secs(10);
            while !path.exists() && Instant::now() < deadline {
                std::thread::sleep(POLL_INTERVAL);
            }
            assert!(
                path.exists(),
                "redirection is shell syntax, so it needs a shell"
            );
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Where there is no shell at all the step is refused by name, not run
    /// wrongly.
    #[cfg(not(any(unix, windows)))]
    #[test]
    fn a_step_without_a_shell_is_refused_by_name() {
        let outcomes = perform(&["true".to_string()], None);
        assert_eq!(outcomes[0].error.as_deref(), Some(NO_SHELL));

        let err = capture("true", None, Duration::from_secs(5), 1024).unwrap_err();
        assert_eq!(err, NO_SHELL);
    }
}
