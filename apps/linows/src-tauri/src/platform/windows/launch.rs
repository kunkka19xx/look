//! Elevated launch via the shell `runas` verb, which triggers the UAC prompt.
//! `open::that` uses the default `open` verb; only `ShellExecuteW` with `runas`
//! requests elevation for the target.

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR};

const ERROR_CANCELLED: isize = 1223;

/// `look-cmd://program[?args]` → (program, args); anything else is the exe itself.
pub fn split_target(path: &str) -> (&str, Option<&str>) {
    match path.strip_prefix("look-cmd://") {
        Some(rest) => match rest.split_once('?') {
            Some((program, args)) => (program, Some(args)),
            None => (rest, None),
        },
        None => (path, None),
    }
}

/// Blocks until the UAC prompt is answered. `Err` on decline or launch failure.
pub fn run_as_admin(program: &str, args: Option<&str>) -> Result<(), String> {
    let verb = HSTRING::from("runas");
    let file = HSTRING::from(program);
    let params = args.map(HSTRING::from);
    // ShellExecuteW returns an HINSTANCE that is only an error indicator: a value
    // greater than 32 means success, anything else is a failure code.
    let hinst = unsafe {
        ShellExecuteW(
            None,
            &verb,
            &file,
            params
                .as_ref()
                .map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr())),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    match hinst.0 as isize {
        code if code > 32 => Ok(()),
        ERROR_CANCELLED => Err("elevation declined".into()),
        code => Err(format!("runas {program:?} failed (code {code})")),
    }
}

#[cfg(test)]
mod tests {
    use super::split_target;

    #[test]
    fn splits_look_cmd_targets() {
        assert_eq!(
            split_target("look-cmd://regedit.exe"),
            ("regedit.exe", None)
        );
        assert_eq!(
            split_target("look-cmd://rundll32.exe?sysdm.cpl,EditEnvironmentVariables"),
            ("rundll32.exe", Some("sysdm.cpl,EditEnvironmentVariables"))
        );
        assert_eq!(
            split_target(r"C:\Windows\notepad.exe"),
            (r"C:\Windows\notepad.exe", None)
        );
    }
}
