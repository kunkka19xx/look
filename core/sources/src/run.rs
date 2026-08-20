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
