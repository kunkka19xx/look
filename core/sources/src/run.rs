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

/// Used when `$SHELL` is unset, which happens for processes the window server
/// starts without a user session environment.
const FALLBACK_SHELL: &str = "/bin/sh";

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
        .map(|step| StepOutcome {
            step: step.clone(),
            error: spawn(step, row).err(),
        })
        .collect()
}

/// The row a step is being performed on, exported so a script can read it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowContext {
    pub id: String,
    pub title: String,
    pub path: String,
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
