//! Windows process listing / kill stubs. Real impl (Toolhelp32 snapshot +
//! TerminateProcess + per-port netstat) lands in M4.

use crate::process::RunningApp;

pub(crate) fn list() -> Vec<RunningApp> {
    // TODO(M4): Toolhelp32Snapshot + Process32First/Next, then merge with .lnk
    // discovery to surface app-friendly names (mirroring the Linux .desktop join).
    Vec::new()
}

pub(crate) fn list_on_port(_port: u16) -> Vec<RunningApp> {
    // TODO(M4): GetExtendedTcpTable + match listening sockets to PIDs.
    Vec::new()
}

pub(crate) fn kill(pid: u32) -> Result<String, String> {
    // TODO(M4): OpenProcess(PROCESS_TERMINATE) + TerminateProcess.
    Err(format!("kill not yet implemented on Windows (pid {pid})"))
}
