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

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Used when `$SHELL` is unset, which happens for processes the window server
/// starts without a user session environment.
const FALLBACK_SHELL: &str = "/bin/sh";

/// How often a captured command is checked for having finished. Short enough
/// that a fast command is not held up, long enough not to spin.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

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
}

/// Substitutes `{id}`, `{title}`, `{path}`, `{dir}`, and `{query}`.
///
/// Every value is quoted on the way in, which is a correctness requirement
/// before it is a safety one: rows come from directory listings and command
/// output, so a folder called `my project` must stay one argument and a row
/// titled `; rm -rf ~` must not execute. A template therefore never quotes its
/// own placeholders.
pub fn expand(template: &str, row: &RowContext) -> String {
    let dir = parent_dir(&row.path);
    template
        .replace("{id}", &quote(&row.id))
        .replace("{title}", &quote(&row.title))
        .replace("{path}", &quote(&row.path))
        .replace("{dir}", &quote(&dir))
        .replace("{query}", &quote(&row.query))
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

/// POSIX single-quoting: everything inside is literal, and an embedded quote is
/// closed, escaped, and reopened.
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn spawn(step: &str, row: Option<&RowContext>) -> Result<(), String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| FALLBACK_SHELL.to_string());
    let mut command = Command::new(&shell);
    command
        .arg("-lc")
        .arg(step)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(row) = row {
        command
            .env(ENV_ID, &row.id)
            .env(ENV_TITLE, &row.title)
            .env(ENV_PATH, &row.path);
        if !row.path.is_empty() {
            command.current_dir(&row.path);
        }
    }

    detach(&mut command);
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("{shell}: {err}"))
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
    let shell = std::env::var("SHELL").unwrap_or_else(|_| FALLBACK_SHELL.to_string());
    let mut spawned = Command::new(&shell)
        .arg("-lc")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(cwd.filter(|dir| !dir.is_empty()).unwrap_or("/"))
        .spawn()
        .map_err(|err| format!("{shell}: {err}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match spawned.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = spawned.kill();
                    let _ = spawned.wait();
                    return Err(format!("timed out after {}s", timeout.as_secs()));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => return Err(err.to_string()),
        }
    }

    let output = spawned.wait_with_output().map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.lines().next().unwrap_or("exited non-zero").trim();
        return Err(reason.to_string());
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.len() > max_bytes {
        // Cut on a char boundary so a multi-byte glyph at the cap does not
        // produce a panic or a replacement character.
        let mut cut = max_bytes;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    Ok(text)
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

#[cfg(not(unix))]
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
        }
    }

    #[test]
    fn placeholders_expand_to_the_selected_row() {
        let expanded = expand("nvim {path} # {title} {query}", &row("/tmp/look"));
        assert_eq!(expanded, "nvim '/tmp/look' # 'look' 'loo'");
    }

    #[test]
    fn a_path_with_spaces_stays_one_argument() {
        assert_eq!(
            expand("code {path}", &row("/tmp/my project")),
            "code '/tmp/my project'"
        );
    }

    #[test]
    fn a_row_named_like_a_command_cannot_execute() {
        // Rows come from directory listings and command output, so this is the
        // difference between quoting and a deleted home directory.
        let mut hostile = row("");
        hostile.title = "; rm -rf ~".into();
        assert_eq!(expand("echo {title}", &hostile), "echo '; rm -rf ~'");
    }

    #[test]
    fn an_embedded_quote_survives_intact() {
        let mut tricky = row("");
        tricky.title = "it's".into();
        assert_eq!(expand("echo {title}", &tricky), "echo 'it'\\''s'");
    }

    #[test]
    fn dir_is_the_row_when_it_is_a_folder_and_its_parent_otherwise() {
        let dir = std::env::temp_dir().join(format!("look-expand-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("probe dir");
        let dir_text = dir.to_str().expect("utf8 path");

        assert_eq!(expand("{dir}", &row(dir_text)), format!("'{dir_text}'"));

        let file = dir.join("probe.txt");
        std::fs::write(&file, "x").expect("probe file");
        assert_eq!(
            expand("{dir}", &row(file.to_str().unwrap())),
            format!("'{dir_text}'"),
            "a file row hands its command the folder it lives in"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_returns_what_the_command_printed() {
        let out = capture("printf 'a\nb\n'", None, Duration::from_secs(5), 1024).unwrap();
        assert_eq!(out, "a\nb\n");
    }

    #[test]
    fn a_command_that_fails_reports_its_first_stderr_line() {
        let err = capture("echo boom >&2; exit 1", None, Duration::from_secs(5), 1024).unwrap_err();
        assert_eq!(err, "boom");
    }

    #[test]
    fn a_hung_command_is_killed_rather_than_waited_on() {
        let err = capture("sleep 30", None, Duration::from_millis(150), 1024).unwrap_err();
        assert!(err.contains("timed out"), "{err}");
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

        // The step is detached, so give it a moment before reading.
        for _ in 0..50 {
            if path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            path.exists(),
            "redirection is shell syntax, so it needs a shell"
        );
        let _ = std::fs::remove_file(&path);
    }
}
