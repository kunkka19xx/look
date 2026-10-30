//! Whether a declared block can work on this machine.
//!
//! A file that parsed perfectly can still do nothing here: it names a folder
//! that only ever existed on the machine the example was written on, or a
//! program that is not installed. The loader cannot call that a `Problem`,
//! because the declaration is valid, so it is reported here instead: per block,
//! naming the key to change and the value as written.
//!
//! Static only. Nothing here runs a declared command. A `do` block's steps are
//! what Enter performs, and a readiness check must not perform them to find out
//! whether they would work.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::collect::expand_home;
use crate::def::{Block, Producer};
use crate::load::is_executable;

/// Why a declared value cannot work here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// The path is not there. Said only when the filesystem said so.
    MissingPath,
    /// The path could not be read at all, so whether it is there is unknown.
    /// Kept apart from `MissingPath` because a sandboxed app is refused
    /// `~/Documents` outright, and calling that missing would send the user off
    /// to change a line that was right all along.
    Unreadable,
    /// A command names a program that is not on `PATH`.
    MissingProgram,
    /// An absolute path under somebody else's home: the file came from another
    /// machine, which is worth saying rather than reporting as merely missing.
    ForeignHome,
}

/// One declared value to change before `block` works here.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unready {
    pub block: String,
    /// The key that declared it: a producer key (`dir`, `file`, `run`, `do`,
    /// `connect`), `cwd`, `preview`, or a verb.
    pub key: &'static str,
    /// As written in the file rather than expanded, because that is the text
    /// the user has to go and find.
    pub value: String,
    pub reason: Reason,
}

impl Unready {
    pub fn message(&self) -> String {
        let Self { key, value, .. } = self;
        match self.reason {
            Reason::MissingPath => format!("`{key}` points at {value}, which is not there"),
            Reason::Unreadable => format!("`{key}` points at {value}, which Look may not read"),
            Reason::MissingProgram => format!("`{key}` needs {value}, which is not installed"),
            Reason::ForeignHome => {
                format!("`{key}` points at {value}, which is another machine's home folder")
            }
        }
    }
}

/// Deduplicated while collecting: one program named by six steps is one thing
/// to install, not six.
type Found = BTreeSet<(&'static str, String, Reason)>;

/// What to change before `block` works on this machine. Empty means every path
/// it reads is there and every program it names is installed.
pub fn check(block: &Block, home: &Path) -> Vec<Unready> {
    let mut found = Found::new();
    let key = block.producer.key();

    match &block.producer {
        Producer::Dir { roots, .. } => {
            for root in roots {
                check_path(root, key, home, &mut found);
            }
        }
        Producer::File { path, .. } => check_path(path, key, home, &mut found),
        Producer::Run { command, cwd, .. } => {
            check_command(command, key, home, &mut found);
            if let Some(cwd) = cwd {
                check_path(cwd, "cwd", home, &mut found);
            }
        }
        Producer::Bundle { steps } => {
            for step in steps {
                check_command(step, key, home, &mut found);
            }
        }
        Producer::Connect { command } => check_command(command, key, home, &mut found),
        // A verb the connector owning the row plans and then runs. Nothing
        // declared here is ours to resolve.
        Producer::Ask { .. } => {}
    }

    if let Some(preview) = &block.preview {
        check_command(preview, "preview", home, &mut found);
    }
    for (verb, command) in block.verbs.declared() {
        check_command(command, verb, home, &mut found);
    }

    found
        .into_iter()
        .map(|(key, value, reason)| Unready {
            block: block.id.clone(),
            key,
            value,
            reason,
        })
        .collect()
}

/// A path the block reads, so it has to be there.
fn check_path(declared: &str, key: &'static str, home: &Path, found: &mut Found) {
    // A row placeholder is filled in from whichever row this runs on, and a
    // shell expansion from whatever the shell makes of it, so neither has one
    // path to check.
    if declared.is_empty() || declared.contains('{') || declared.contains('$') {
        return;
    }
    let reason = match foreign_home(declared, home) {
        Some(reason) => Some(reason),
        None => missing(&expand_home(declared, home)),
    };
    if let Some(reason) = reason {
        found.insert((key, declared.to_string(), reason));
    }
}

/// `None` when the path is there.
fn missing(path: &Path) -> Option<Reason> {
    match fs::metadata(path) {
        Ok(_) => None,
        Err(err) if err.kind() == ErrorKind::NotFound => Some(Reason::MissingPath),
        // Denied, a dead mount, a symlink with nothing at the end: Look cannot
        // say the path is absent, only that it could not look.
        Err(_) => Some(Reason::Unreadable),
    }
}

/// `/Users/someone/...` where someone is not this user.
fn foreign_home(declared: &str, home: &Path) -> Option<Reason> {
    const HOME_ROOTS: [&str; 2] = ["/Users/", "/home/"];

    let mine = home.file_name()?.to_str()?;
    for root in HOME_ROOTS {
        let Some(rest) = declared.strip_prefix(root) else {
            continue;
        };
        let user = rest.split('/').next()?;
        if !user.is_empty() && user != mine {
            return Some(Reason::ForeignHome);
        }
    }
    None
}

/// A declared command: every program it runs has to be installed, and a
/// home-relative path it reads has to be there.
fn check_command(command: &str, key: &'static str, home: &Path, found: &mut Found) {
    let segments = segments(command);
    let mut start = 0;
    while start < segments.len() {
        let mut end = start + 1;
        while end < segments.len() && segments[end].alternative {
            end += 1;
        }
        check_alternatives(&segments[start..end], key, home, found);
        start = end;
    }
}

/// One intent: a command, plus the `||` fallbacks written for it. A fallback is
/// there precisely because the first choice is not on every machine, so
/// `xdg-open x || open x` is missing nothing until both are.
fn check_alternatives(group: &[Segment], key: &'static str, home: &Path, found: &mut Found) {
    let mut missing = Found::new();
    let mut runs = false;
    for segment in group {
        let words = without_redirects(&segment.words);
        let Some(program) = words.first() else {
            continue;
        };
        let mut one = Found::new();
        check_program(program, key, home, &mut one);
        if one.is_empty() {
            runs = true;
        } else {
            missing.extend(one);
        }
    }
    if !runs {
        found.extend(missing);
    }

    // Path arguments, only where one command reads them: in a fallback chain
    // the path that matters belongs to whichever alternative ends up running.
    let [segment] = group else {
        return;
    };
    let words = without_redirects(&segment.words);
    let mut words = words.into_iter();
    let Some(program) = words.next() else {
        return;
    };
    // `mkdir -p ~/vault/Daily` is how that folder comes to exist. Its absence
    // is the point of the step, not a mistake in it.
    if creates(program) {
        return;
    }
    for word in words {
        let word = unquote(word);
        if word.starts_with("~/") {
            check_path(word, key, home, found);
        }
    }
}

/// Redirect operators and the targets they name removed. `>> ~/log` brings
/// that file into being, and `2>&1` is not a program.
fn without_redirects(words: &[String]) -> Vec<&String> {
    let mut kept = Vec::new();
    let mut target = false;
    for word in words {
        if target {
            target = false;
            continue;
        }
        if is_redirect(word) {
            // `>> file` names its target in the next word, `>>file` in its own.
            target = word.ends_with(['>', '<']);
            continue;
        }
        kept.push(word);
    }
    kept
}

/// `>`, `>>`, `<`, and the same with a file descriptor in front.
fn is_redirect(word: &str) -> bool {
    let rest = match word.as_bytes().first() {
        Some(b'0' | b'1' | b'2') => &word[1..],
        _ => word,
    };
    rest.starts_with('>') || rest.starts_with('<')
}

/// The first word of a segment, which is the program it runs.
fn check_program(word: &str, key: &'static str, home: &Path, found: &mut Found) {
    // Filled in from the row, assigned to the environment, or quoted into
    // something no static read can resolve.
    if word.is_empty()
        || word.contains('{')
        || word.contains('$')
        || word.contains('=')
        || word.starts_with(['\'', '"', '(', '-'])
    {
        return;
    }
    if builtin(word) {
        return;
    }
    // Named by path rather than by name: a file to find, not a lookup.
    if word.contains('/') {
        check_path(word, key, home, found);
        return;
    }
    if !on_path(word) {
        found.insert((key, word.to_string(), Reason::MissingProgram));
    }
}

/// Shell builtins and keywords. Some are also on `PATH` and some never are, but
/// either way the shell answers them itself, so looking is pointless.
fn builtin(word: &str) -> bool {
    const BUILTINS: [&str; 41] = [
        ".", ":", "[", "alias", "break", "case", "cd", "command", "continue", "declare", "do",
        "done", "echo", "elif", "else", "esac", "eval", "exec", "exit", "export", "false", "fi",
        "for", "function", "if", "in", "let", "local", "printf", "pwd", "read", "readonly",
        "return", "set", "shift", "source", "test", "then", "true", "until", "while",
    ];

    BUILTINS.contains(&word)
}

/// Commands whose path argument is one they bring into being or take away.
/// Checking those for existence would report the step's own purpose as a fault.
fn creates(program: &str) -> bool {
    const CREATORS: [&str; 10] = [
        "cp", "install", "ln", "mkdir", "mkfifo", "mv", "rm", "rmdir", "tee", "touch",
    ];

    CREATORS.contains(&program.rsplit('/').next().unwrap_or(program))
}

/// A matched pair of surrounding quotes removed, so a quoted path is still
/// recognised as one.
fn unquote(word: &str) -> &str {
    for quote in ['\'', '"'] {
        if let Some(inner) = word
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    word
}

/// One top-level command in a chain, plus how it was joined to the one before.
#[derive(Debug)]
struct Segment {
    words: Vec<String>,
    /// `||` joined it to the previous segment, so it is that command's
    /// fallback rather than a command of its own.
    alternative: bool,
}

/// The command split into its top-level segments, each a list of words.
///
/// Quote-aware, so the `|` in `awk -F'|'` is a field separator rather than a
/// pipe and the script after it is one word rather than forty. Quotes are kept
/// in the word, because whether a word was quoted is what decides it cannot be
/// resolved statically.
fn segments(command: &str) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut alternative = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            // Whatever follows is literal, `\;` and `\ ` included.
            '\\' => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            // A backslash is literal inside single quotes, which is what makes
            // `awk '\t'` a tab for awk rather than an escape for the shell.
            '\'' => {
                word.push(ch);
                for inner in chars.by_ref() {
                    word.push(inner);
                    if inner == '\'' {
                        break;
                    }
                }
            }
            '"' | '`' => {
                word.push(ch);
                let mut escaped = false;
                for inner in chars.by_ref() {
                    word.push(inner);
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == ch {
                        break;
                    }
                }
            }
            // A command substitution is one word nothing static can resolve,
            // not a subshell to look inside: `~/vault/$(date +%F).md` is a
            // path that does not exist yet and is not meant to.
            '$' if chars.peek() == Some(&'(') => {
                word.push(ch);
                let mut depth = 0usize;
                for inner in chars.by_ref() {
                    word.push(inner);
                    match inner {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            // A heredoc. Everything after it is input for the command that
            // opened it, and reading an AppleScript as a list of shell
            // commands finds a missing program on every line of it.
            '<' if chars.peek() == Some(&'<') => break,
            ';' | '\n' => end(&mut out, &mut words, &mut word, &mut alternative, false),
            '|' => {
                let fallback = chars.peek() == Some(&'|');
                if fallback {
                    chars.next();
                }
                end(&mut out, &mut words, &mut word, &mut alternative, fallback);
            }
            '&' => {
                if chars.peek() == Some(&'&') {
                    chars.next();
                } else if word.ends_with('>') {
                    // `2>&1` is one redirect, not a backgrounded command.
                    word.push(ch);
                    continue;
                }
                end(&mut out, &mut words, &mut word, &mut alternative, false);
            }
            // A subshell's parentheses are not part of the program inside it.
            '(' | ')' => push(&mut words, &mut word),
            _ if ch.is_whitespace() => push(&mut words, &mut word),
            _ => word.push(ch),
        }
    }

    push(&mut words, &mut word);
    if !words.is_empty() {
        out.push(Segment { words, alternative });
    }
    out
}

/// Closes the segment being built. `fallback` says how the *next* segment is
/// joined to it.
fn end(
    out: &mut Vec<Segment>,
    words: &mut Vec<String>,
    word: &mut String,
    alternative: &mut bool,
    fallback: bool,
) {
    push(words, word);
    if !words.is_empty() {
        out.push(Segment {
            words: std::mem::take(words),
            alternative: *alternative,
        });
    }
    *alternative = fallback;
}

fn push(words: &mut Vec<String>, word: &mut String) {
    if !word.is_empty() {
        words.push(std::mem::take(word));
    }
}

/// Whether a program is installed here.
///
/// Public because a requirement can arrive without a block behind it: a catalog
/// entry saying what an example needs before you install it is a bare program
/// name. Sharing this with the block check is what stops a browser and the
/// Extend screen disagreeing about whether `docker` is on this machine.
pub fn installed(program: &str) -> bool {
    on_path(program)
}

/// Not `which`: it is not installed everywhere, and asking a process whether a
/// program exists would spawn one per declared step.
fn on_path(program: &str) -> bool {
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|dir| dir.is_absolute() && found_in(&dir, program))
    })
}

#[cfg(not(windows))]
fn found_in(dir: &Path, program: &str) -> bool {
    let candidate = dir.join(program);
    candidate.is_file() && is_executable(&candidate)
}

/// `cmd` finds `docker` by trying `docker.exe`, `docker.cmd` and the rest of
/// `PATHEXT`, so a bare name has to be tried the same way here or every program
/// on Windows reads as missing.
#[cfg(windows)]
fn found_in(dir: &Path, program: &str) -> bool {
    const FALLBACK_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

    let named = dir.join(program);
    if named.is_file() && is_executable(&named) {
        return true;
    }
    let pathext = env::var("PATHEXT").unwrap_or_else(|_| FALLBACK_PATHEXT.to_string());
    pathext.split(';').any(|extension| {
        let extension = extension.trim().trim_start_matches('.');
        !extension.is_empty() && dir.join(format!("{program}.{extension}")).is_file()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::parse_file;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-sources-check-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The first block of `toml`, checked against `home`.
    fn checked(toml: &str, home: &Path) -> Vec<Unready> {
        let parsed = parse_file(toml).expect("parses");
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        let block = parsed.blocks.first().expect("a block");
        check(block, home)
    }

    fn reasons(found: &[Unready]) -> Vec<(&str, &str, Reason)> {
        found
            .iter()
            .map(|one| (one.key, one.value.as_str(), one.reason))
            .collect()
    }

    #[test]
    fn a_missing_dir_names_the_key_and_the_value_as_written() {
        let home = TempDir::new("missing-dir");
        let found = checked(
            r#"
            [notes]
            dir = "~/Documents/vault"
            "#,
            &home.0,
        );
        assert_eq!(
            reasons(&found),
            [("dir", "~/Documents/vault", Reason::MissingPath)]
        );
        assert_eq!(
            found[0].message(),
            "`dir` points at ~/Documents/vault, which is not there"
        );
        assert_eq!(found[0].block, "notes");
    }

    #[test]
    fn a_dir_that_is_there_is_not_reported() {
        let home = TempDir::new("present-dir");
        fs::create_dir_all(home.0.join("Documents/vault")).expect("vault");
        let found = checked(
            r#"
            [notes]
            dir = "~/Documents/vault"
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_path_argument_of_a_command_is_checked_too() {
        let home = TempDir::new("run-path");
        let found = checked(
            r#"
            [projects]
            run = "awk -F'|' '{ print $1 }' ~/.projects"
            "#,
            &home.0,
        );
        // The awk script is quoted and holds both a brace and a `$`, so it is
        // never mistaken for a program or a path.
        assert_eq!(
            reasons(&found),
            [("run", "~/.projects", Reason::MissingPath)]
        );
    }

    #[test]
    fn a_pipe_inside_quotes_does_not_start_a_segment() {
        let found = segments("awk -F'|' '{ print }' ~/.projects");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].words[0], "awk");
        assert_eq!(found[0].words[1], "-F'|'");
        assert_eq!(found[0].words.len(), 4, "{found:?}");
    }

    #[test]
    fn each_segment_of_a_chain_contributes_its_program() {
        let found = segments("mkdir -p ~/a && open -g x; ls | wc -l");
        let programs: Vec<&str> = found.iter().map(|one| one.words[0].as_str()).collect();
        assert_eq!(programs, ["mkdir", "open", "ls", "wc"]);
        assert!(found.iter().all(|one| !one.alternative), "{found:?}");
    }

    #[test]
    fn a_command_substitution_stays_inside_the_word_it_is_part_of() {
        let found = segments("echo hi >> ~/vault/$(date +%F).md");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].words,
            ["echo", "hi", ">>", "~/vault/$(date +%F).md"]
        );
    }

    #[test]
    fn a_heredoc_body_is_not_read_as_more_commands() {
        let found =
            segments("osascript <<'APPLESCRIPT'\ntell application \"Mail\" to quit\nAPPLESCRIPT\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].words, ["osascript"]);
    }

    #[test]
    fn a_fallback_is_marked_as_one_and_a_pipe_is_not() {
        let found = segments("xdg-open x || open x | cat");
        let marks: Vec<bool> = found.iter().map(|one| one.alternative).collect();
        assert_eq!(marks, [false, true, false]);
    }

    #[test]
    fn nothing_is_missing_while_one_fallback_runs() {
        let home = TempDir::new("fallback");
        let found = checked(
            r#"
            [history]
            dir  = "~"
            open = "command -v look-no-such-opener >/dev/null 2>&1 && look-no-such-opener {id} || cat {id}"
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_fallback_chain_is_reported_when_none_of_it_runs() {
        let home = TempDir::new("no-fallback");
        let found = checked(
            r#"
            [history]
            dir  = "~"
            open = "look-no-such-opener {id} || look-no-other-opener {id}"
            "#,
            &home.0,
        );
        assert_eq!(
            reasons(&found),
            [
                ("open", "look-no-other-opener", Reason::MissingProgram),
                ("open", "look-no-such-opener", Reason::MissingProgram),
            ]
        );
    }

    #[test]
    fn a_missing_program_is_reported_once_however_many_steps_name_it() {
        let home = TempDir::new("missing-program");
        let found = checked(
            r#"
            [deploy]
            do = ["look-no-such-program build", "look-no-such-program push"]
            "#,
            &home.0,
        );
        assert_eq!(
            reasons(&found),
            [("do", "look-no-such-program", Reason::MissingProgram)]
        );
    }

    #[test]
    fn a_creating_step_does_not_report_the_path_it_creates() {
        let home = TempDir::new("creator");
        let found = checked(
            r#"
            [daily]
            do = ["mkdir -p ~/vault/Daily && touch ~/vault/Daily/today.md"]
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_redirect_target_is_not_reported() {
        let home = TempDir::new("redirect");
        let found = checked(
            r#"
            [log]
            do = ["printf hello >> ~/vault/log.md"]
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_row_placeholder_is_never_checked() {
        let home = TempDir::new("placeholder");
        let found = checked(
            r#"
            [notes]
            dir     = "~"
            preview = "sed -n 1,80p {path}"
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn another_users_home_is_called_that_rather_than_missing() {
        let home = TempDir::new("foreign");
        let found = checked(
            r#"
            [notes]
            dir = "/Users/someone-else/Documents/vault"
            "#,
            &home.0,
        );
        assert_eq!(
            reasons(&found),
            [(
                "dir",
                "/Users/someone-else/Documents/vault",
                Reason::ForeignHome
            )]
        );
    }

    #[test]
    fn a_program_named_by_path_is_checked_as_a_file() {
        let home = TempDir::new("program-path");
        let found = checked(
            r#"
            [notes]
            dir     = "~"
            preview = "~/.look/bin/note-preview {arg}"
            "#,
            &home.0,
        );
        assert_eq!(
            reasons(&found),
            [("preview", "~/.look/bin/note-preview", Reason::MissingPath)]
        );
    }

    #[test]
    fn a_declared_verb_is_checked_under_its_own_key() {
        let home = TempDir::new("verb");
        let found = checked(
            r#"
            [notes]
            dir  = "~"
            edit = "look-no-such-editor {path}"
            "#,
            &home.0,
        );
        assert_eq!(
            reasons(&found),
            [("edit", "look-no-such-editor", Reason::MissingProgram)]
        );
    }

    #[test]
    fn a_bare_program_name_can_be_probed_without_a_block() {
        // `sh` is on every machine this builds for; the second name is not a
        // program anywhere.
        assert!(installed("sh"));
        assert!(!installed("look-no-such-program"));
    }

    #[test]
    fn a_shell_builtin_is_not_a_missing_program() {
        let home = TempDir::new("builtin");
        let found = checked(
            r#"
            [say]
            do = ["printf hello", "cd /tmp", "true"]
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn an_environment_assignment_is_not_a_program() {
        let home = TempDir::new("env");
        let found = checked(
            r#"
            [build]
            do = ["RUST_LOG=debug cargo --version"]
            "#,
            &home.0,
        );
        assert!(
            found.iter().all(|one| one.value != "RUST_LOG=debug"),
            "{found:?}"
        );
    }

    #[test]
    fn an_ask_block_declares_nothing_to_check() {
        let home = TempDir::new("ask");
        let found = checked(
            r#"
            [archive]
            ask = "archive"
            "#,
            &home.0,
        );
        assert!(found.is_empty(), "{found:?}");
    }
}
