//! Classic Win32 / MMC applets that Windows Search surfaces but `ms-settings:`
//! doesn't cover (env vars, Device Manager, Services, Registry, Task Manager,
//! etc.). The Tauri side handles `look-cmd://` paths by splitting on the first
//! `?` into program + args and spawning via `Command::new`.
//!
//! Format:
//!   - `program` alone        → ShellExecute resolves the .cpl/.msc/.exe by name
//!   - `program?args`         → spawn with args (e.g. rundll32.exe with a DLL+entry)
//!
//! Encoded into the candidate path as `look-cmd://program[?args]`.

#[derive(Clone, Copy)]
pub(crate) struct ControlPanelEntry {
    pub title: &'static str,
    pub program: &'static str,
    /// `None` for direct file launches (.msc/.cpl/.exe). Single string passed
    /// verbatim as a single argv entry for rundll32-style commands.
    pub args: Option<&'static str>,
    pub candidate_id_suffix: &'static str,
    pub aliases: &'static str,
}

pub(crate) const CONTROL_PANEL_CATALOG: &[ControlPanelEntry] = &[
    ControlPanelEntry {
        title: "Edit the system environment variables",
        program: "rundll32.exe",
        args: Some("sysdm.cpl,EditEnvironmentVariables"),
        candidate_id_suffix: "windows.cpl.env",
        aliases: "settings system environment variables path env user account",
    },
    ControlPanelEntry {
        title: "Device Manager",
        program: "devmgmt.msc",
        args: None,
        candidate_id_suffix: "windows.cpl.devmgmt",
        aliases: "settings device manager hardware driver",
    },
    ControlPanelEntry {
        title: "Services",
        program: "services.msc",
        args: None,
        candidate_id_suffix: "windows.cpl.services",
        aliases: "settings services daemon background start stop",
    },
    ControlPanelEntry {
        title: "Registry Editor",
        program: "regedit.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.regedit",
        aliases: "settings registry editor regedit hkey",
    },
    ControlPanelEntry {
        title: "Task Manager",
        program: "taskmgr.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.taskmgr",
        aliases: "settings task manager processes cpu memory",
    },
    ControlPanelEntry {
        title: "Event Viewer",
        program: "eventvwr.msc",
        args: None,
        candidate_id_suffix: "windows.cpl.eventvwr",
        aliases: "settings event viewer logs system application",
    },
    ControlPanelEntry {
        title: "Disk Management",
        program: "diskmgmt.msc",
        args: None,
        candidate_id_suffix: "windows.cpl.diskmgmt",
        aliases: "settings disk management partition volume format",
    },
    ControlPanelEntry {
        title: "Computer Management",
        program: "compmgmt.msc",
        args: None,
        candidate_id_suffix: "windows.cpl.compmgmt",
        aliases: "settings computer management mmc admin",
    },
    ControlPanelEntry {
        title: "Network Connections",
        program: "ncpa.cpl",
        args: None,
        candidate_id_suffix: "windows.cpl.ncpa",
        aliases: "settings network connections adapter wifi ethernet",
    },
    ControlPanelEntry {
        title: "Programs and Features",
        program: "appwiz.cpl",
        args: None,
        candidate_id_suffix: "windows.cpl.appwiz",
        aliases: "settings programs features uninstall install remove",
    },
    ControlPanelEntry {
        title: "System Properties",
        program: "sysdm.cpl",
        args: None,
        candidate_id_suffix: "windows.cpl.sysdm",
        aliases: "settings system properties advanced startup recovery",
    },
    ControlPanelEntry {
        title: "User Accounts (advanced)",
        program: "netplwiz.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.netplwiz",
        aliases: "settings user accounts netplwiz password autologon",
    },
    ControlPanelEntry {
        title: "System Configuration",
        program: "msconfig.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.msconfig",
        aliases: "settings system configuration msconfig boot startup",
    },
    ControlPanelEntry {
        title: "System Information",
        program: "msinfo32.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.msinfo32",
        aliases: "settings system information hardware specs cpu ram",
    },
    ControlPanelEntry {
        title: "Resource Monitor",
        program: "resmon.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.resmon",
        aliases: "settings resource monitor cpu memory disk network",
    },
    ControlPanelEntry {
        title: "DirectX Diagnostic Tool",
        program: "dxdiag.exe",
        args: None,
        candidate_id_suffix: "windows.cpl.dxdiag",
        aliases: "settings directx diagnostic gpu graphics",
    },
    ControlPanelEntry {
        title: "Performance Monitor",
        program: "perfmon.msc",
        args: None,
        candidate_id_suffix: "windows.cpl.perfmon",
        aliases: "settings performance monitor counters",
    },
];

pub(crate) const CONTROL_PANEL_SCHEME: &str = "look-cmd://";

/// Build the synthetic path for a Control Panel candidate.
pub(crate) fn target_path(entry: &ControlPanelEntry) -> String {
    match entry.args {
        Some(args) => format!("{CONTROL_PANEL_SCHEME}{}?{}", entry.program, args),
        None => format!("{CONTROL_PANEL_SCHEME}{}", entry.program),
    }
}
