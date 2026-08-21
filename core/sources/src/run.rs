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
//! A step is POSIX shell text down to its punctuation - `&&`, `>`, and the
//! single quotes `expand` wraps every placeholder in. A `$SHELL` reading
//! another language is passed over for `/bin/sh`; Windows, having none, refuses
//! the step rather than hand `cmd` arguments it would mangle.

use std::io::Read;
use std::process::{Command, Stdio};

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

/// Said instead of running anything, so the step reads as unsupported rather
/// than as a missing program.
#[cfg(not(unix))]
const NO_POSIX_SHELL: &str = "steps are POSIX shell commands, which this platform has no shell for";

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
    command.arg("-lc").arg(step);
    Ok(command)
}

#[cfg(not(unix))]
fn shell_command(_step: &str) -> Result<Command, String> {
    Err(NO_POSIX_SHELL.to_string())
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
        .stderr(Stdio::null())
        // An AppImage runtime points this at its own bundled libs, and a host
        // binary resolving against them dies on a symbol lookup. A step is the
        // user's own command, so it must see the host's loader path, not ours.
        .env_remove("LD_LIBRARY_PATH");

    if let Some(row) = row {
        command
            .env(ENV_ID, &row.id)
            .env(ENV_TITLE, &row.title)
            .env(ENV_PATH, &row.path);
        // The row's FOLDER, never the row itself: `path` is a file for most
        // rows, and `current_dir` on a file makes every spawn fail with
        // ENOTDIR. Same rule as `{dir}`.
        let dir = parent_dir(&row.path);
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
        .current_dir(cwd.filter(|dir| !dir.is_empty()).unwrap_or("/"));
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
                    let _ = spawned.kill();
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

    /// Off Unix the step is refused by name, not run wrongly.
    #[cfg(not(unix))]
    #[test]
    fn a_step_without_a_posix_shell_is_refused_by_name() {
        let outcomes = perform(&["true".to_string()], None);
        assert_eq!(outcomes[0].error.as_deref(), Some(NO_POSIX_SHELL));

        let err = capture("true", None, Duration::from_secs(5), 1024).unwrap_err();
        assert_eq!(err, NO_POSIX_SHELL);
    }
}
