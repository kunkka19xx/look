//! Windows /sys collector. Mirrors the section layout of the Linux/macOS
//! versions: OS, memory, CPU, battery, uptime, disk. CPU live-usage is skipped
//! on Windows - getting it requires two GetSystemTimes calls separated by a
//! delay, and we don't want the /sys command to block.

use crate::sysinfo::SysInfoEntry;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_VALUE_TYPE, RegCloseKey, RegOpenKeyExW,
    RegQueryValueExW,
};
use windows::Win32::System::SystemInformation::{
    GetSystemInfo, GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows::core::PCWSTR;

pub fn collect() -> Vec<Vec<SysInfoEntry>> {
    let mut sections: Vec<Vec<SysInfoEntry>> = Vec::new();

    // Section 1: OS
    {
        let mut s = Vec::new();
        if let Some(host) = std::env::var("COMPUTERNAME").ok().filter(|v| !v.is_empty()) {
            s.push(SysInfoEntry {
                label: "Host".into(),
                value: host,
            });
        }
        let (product, build, display) = read_os_version();
        // ProductName in the registry was frozen at "Windows 10" for early Win11
        // builds. The 22000+ build number is the canonical Win11 marker.
        let os = match (product.as_deref(), build) {
            (Some(p), Some(b)) if b >= 22000 && p.contains("Windows 10") => {
                p.replacen("Windows 10", "Windows 11", 1)
            }
            (Some(p), _) => p.to_string(),
            (None, _) => "Windows".to_string(),
        };
        s.push(SysInfoEntry {
            label: "OS".into(),
            value: os,
        });
        if let Some(b) = build {
            let value = match display {
                Some(d) => format!("{b} ({d})"),
                None => b.to_string(),
            };
            s.push(SysInfoEntry {
                label: "Build".into(),
                value,
            });
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 2: Memory
    {
        let mut s = Vec::new();
        if let Some(m) = read_memory_status() {
            let total_gb = m.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0);
            let used_mb = (m.ullTotalPhys - m.ullAvailPhys) / (1024 * 1024);
            s.push(SysInfoEntry {
                label: "Total".into(),
                value: format!("{:.1} GB", total_gb),
            });
            s.push(SysInfoEntry {
                label: "Used".into(),
                value: format!("{} MB", used_mb),
            });
            s.push(SysInfoEntry {
                label: "Load".into(),
                value: format!("{}%", m.dwMemoryLoad),
            });
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 3: CPU
    {
        let mut s = Vec::new();
        if let Some(model) = read_cpu_name() {
            s.push(SysInfoEntry {
                label: "CPU".into(),
                value: model,
            });
        }
        let cores = read_cpu_cores();
        if cores > 0 {
            s.push(SysInfoEntry {
                label: "Cores".into(),
                value: format!("{}", cores),
            });
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 4: Battery
    if let Some(power) = read_power_status() {
        // 255 = unknown / no battery; skip the section entirely.
        if power.BatteryLifePercent != 255 {
            let mut s = Vec::new();
            s.push(SysInfoEntry {
                label: "Charge".into(),
                value: format!("{}%", power.BatteryLifePercent),
            });
            let status = match power.ACLineStatus {
                0 => "Discharging",
                1 => "Charging",
                _ => "Unknown",
            };
            s.push(SysInfoEntry {
                label: "Status".into(),
                value: status.into(),
            });
            sections.push(s);
        }
    }

    // Section 5: Uptime
    {
        let ms = unsafe { GetTickCount64() };
        let total_secs = ms / 1000;
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let mins = (total_secs % 3600) / 60;
        let val = if days > 0 {
            format!("{}d {}h {}m", days, hours, mins)
        } else {
            format!("{}h {}m", hours, mins)
        };
        sections.push(vec![SysInfoEntry {
            label: "Time".into(),
            value: val,
        }]);
    }

    // Section 6: Disk (system drive)
    {
        let mut s = Vec::new();
        let system_drive = std::env::var("SystemDrive")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "C:".into());
        let root = format!("{}\\", system_drive.trim_end_matches('\\'));
        if let Some((free, total)) = read_disk_space(&root) {
            let used = total.saturating_sub(free);
            let used_gb = used as f64 / (1024.0 * 1024.0 * 1024.0);
            let total_gb = total as f64 / (1024.0 * 1024.0 * 1024.0);
            let pct = if total > 0 {
                ((used as f64 / total as f64) * 100.0).round() as u64
            } else {
                0
            };
            s.push(SysInfoEntry {
                label: system_drive,
                value: format!("{:.0} GB / {:.0} GB ({}%)", used_gb, total_gb, pct),
            });
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    sections
}

fn read_memory_status() -> Option<MEMORYSTATUSEX> {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    if unsafe { GlobalMemoryStatusEx(&mut m) }.is_ok() {
        Some(m)
    } else {
        None
    }
}

fn read_cpu_cores() -> u32 {
    let mut info = SYSTEM_INFO::default();
    unsafe { GetSystemInfo(&mut info) };
    info.dwNumberOfProcessors
}

fn read_power_status() -> Option<SYSTEM_POWER_STATUS> {
    let mut s = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut s) }.is_ok() {
        Some(s)
    } else {
        None
    }
}

fn read_disk_space(root: &str) -> Option<(u64, u64)> {
    let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let mut free: u64 = 0;
    let mut total: u64 = 0;
    let res = unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            None,
            Some(&mut total),
            Some(&mut free),
        )
    };
    if res.is_ok() {
        Some((free, total))
    } else {
        None
    }
}

fn read_os_version() -> (Option<String>, Option<u32>, Option<String>) {
    let product = read_reg_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "ProductName",
    );
    let build = read_reg_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "CurrentBuild",
    )
    .and_then(|s| s.parse::<u32>().ok());
    let display = read_reg_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        "DisplayVersion",
    );
    (product, build, display)
}

fn read_cpu_name() -> Option<String> {
    read_reg_string(
        HKEY_LOCAL_MACHINE,
        r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
        "ProcessorNameString",
    )
    .map(|s| s.trim().to_string())
}

fn read_reg_string(root: HKEY, subkey: &str, name: &str) -> Option<String> {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

    let mut hkey = HKEY::default();
    let open = unsafe { RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), None, KEY_READ, &mut hkey) };
    if open.0 != 0 {
        return None;
    }

    // Query size first.
    let mut data_type = REG_VALUE_TYPE(0);
    let mut size: u32 = 0;
    let query_size = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut data_type),
            None,
            Some(&mut size),
        )
    };
    if query_size.0 != 0 || size == 0 {
        let _ = unsafe { RegCloseKey(hkey) };
        return None;
    }

    let mut buf = vec![0u8; size as usize];
    let mut out_size = size;
    let query = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut data_type),
            Some(buf.as_mut_ptr()),
            Some(&mut out_size),
        )
    };
    let _ = unsafe { RegCloseKey(hkey) };
    if query.0 != 0 {
        return None;
    }

    // REG_SZ / REG_EXPAND_SZ are wide-char strings; trim trailing NUL pairs.
    let len_chars = (out_size as usize) / 2;
    let wide: &[u16] = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, len_chars) };
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    Some(String::from_utf16_lossy(&wide[..end]))
}
