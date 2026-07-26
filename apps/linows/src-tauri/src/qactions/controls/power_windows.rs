//! System power actions (restart, shut down) for Windows. Action ids:
//! `"restart"`, `"shutdown"`.
//!
//! Windows peer of `power.rs`. `ExitWindowsEx` performs the reboot / power-off,
//! but needs the `SeShutdownPrivilege` enabled on the process token first, so
//! this adjusts the token then calls it. The launchpad gates both behind an
//! inline confirm before the intent reaches here. Button-only.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Shutdown::{
    EWX_FORCEIFHUNG, EWX_POWEROFF, EWX_REBOOT, EXIT_WINDOWS_FLAGS, ExitWindowsEx, SHUTDOWN_REASON,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::w;

/// Restarts the machine. Button-only. Action id: `"restart"`.
pub struct RestartControl;

/// Shuts the machine down. Button-only. Action id: `"shutdown"`.
pub struct ShutdownControl;

impl SystemControl for RestartControl {
    fn state(&self) -> ActionState {
        present()
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        power(
            intent,
            EWX_REBOOT | EWX_FORCEIFHUNG,
            "Restart",
            "Restarting…",
            "Could not restart",
        )
    }
}

impl SystemControl for ShutdownControl {
    fn state(&self) -> ActionState {
        present()
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        power(
            intent,
            EWX_POWEROFF | EWX_FORCEIFHUNG,
            "Shut Down",
            "Shutting down…",
            "Could not shut down",
        )
    }
}

/// Buttons carry no on/off value; an empty value marks them present so the
/// launchpad renders them wired.
fn present() -> ActionState {
    ActionState::Value {
        value: String::new(),
    }
}

/// Enable the shutdown privilege then call `ExitWindowsEx`, mapping the outcome
/// to the tile's banner. Rejects non-`Run` intents (these are buttons).
fn power(
    intent: ActionIntent,
    flags: EXIT_WINDOWS_FLAGS,
    name: &str,
    ok: &str,
    fail: &str,
) -> ActionOutcome {
    if intent != ActionIntent::Run {
        return ActionOutcome::Failed {
            message: format!("{name} has no toggle"),
        };
    }
    if !enable_shutdown_privilege() {
        return ActionOutcome::Failed {
            message: format!("{fail} (permission denied)"),
        };
    }
    // SHUTDOWN_REASON(0) is "other, unplanned"; the flag details aren't surfaced.
    if unsafe { ExitWindowsEx(flags, SHUTDOWN_REASON(0)) }.is_ok() {
        ActionOutcome::Ok {
            banner: Some(ok.to_string()),
        }
    } else {
        ActionOutcome::Failed {
            message: fail.to_string(),
        }
    }
}

/// Grant `SeShutdownPrivilege` to the current process token. Best-effort; returns
/// whether the adjustment call succeeded.
fn enable_shutdown_privilege() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return false;
        }

        let mut luid = LUID::default();
        let looked_up = LookupPrivilegeValueW(None, w!("SeShutdownPrivilege"), &mut luid).is_ok();

        let ok = looked_up && {
            let privileges = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None).is_ok()
        };

        let _ = CloseHandle(token);
        ok
    }
}
