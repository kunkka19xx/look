//! Quoting for the languages a composed action passes through.

/// POSIX single-quoting: close, escape, reopen.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// `cmd.exe` quoting: wrap in double quotes, double any inside.
///
/// Quoted state is what makes `&`, `|`, `>` and `^` inert, and a doubled quote
/// leaves and re-enters it in one step, so no value can end the argument or
/// start a command of its own. `%NAME%` still expands: the command line has no
/// escape for it, and that is the one thing quoting cannot make literal.
pub fn cmd_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub fn applescript_quote(value: &str) -> String {
    // Backslash first, or the quotes escaped next would have theirs doubled.
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    const HOSTILE: &[&str] = &[
        "plain",
        "/tmp/my project",
        "it's",
        "; rm -rf ~",
        "$(whoami)",
        "`id`",
        "a\"b",
        "back\\slash",
        "new\nline",
        "* ? [glob]",
        "'",
        "''",
    ];

    #[cfg(unix)]
    fn echoed_through_sh(script: &str) -> String {
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .output()
            .expect("running /bin/sh");
        assert!(
            output.status.success(),
            "shell rejected: {script}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8 stdout")
    }

    #[cfg(unix)]
    #[test]
    fn a_real_shell_reads_back_exactly_what_was_quoted() {
        for value in HOSTILE {
            let script = format!("printf '%s' {}", shell_quote(value));
            assert_eq!(&echoed_through_sh(&script), value, "quoting {value:?}");
        }
    }

    /// Hosting a TTY editor quotes an already-quoted command.
    #[cfg(unix)]
    #[test]
    fn nesting_survives_two_rounds() {
        for value in HOSTILE {
            let inner = format!("printf '%s' {}", shell_quote(value));
            let outer = format!("/bin/sh -c {}", shell_quote(&inner));
            assert_eq!(&echoed_through_sh(&outer), value, "nesting {value:?}");
        }
    }

    #[test]
    fn applescript_escapes_backslash_before_quote() {
        assert_eq!(applescript_quote(r#"a\"b"#), r#""a\\\"b""#);
    }
}
