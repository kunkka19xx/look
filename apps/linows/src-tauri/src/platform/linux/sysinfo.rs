//! Linux /sys collector. Reads /proc and /sys + spawns `df` for disk usage.

use crate::sysinfo::SysInfoEntry;

pub fn collect() -> Vec<Vec<SysInfoEntry>> {
    let mut sections: Vec<Vec<SysInfoEntry>> = Vec::new();

    // Section 1: OS
    {
        let mut s = Vec::new();
        if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
            s.push(SysInfoEntry {
                label: "Host".into(),
                value: h.trim().to_string(),
            });
        }
        if let Ok(output) = super::host_command("uname").arg("-r").output() {
            let kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
            s.push(SysInfoEntry {
                label: "Kernel".into(),
                value: kernel,
            });
        }
        if let Ok(release) = std::fs::read_to_string("/etc/os-release")
            && let Some(name) = release
                .lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .and_then(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_string())
        {
            s.push(SysInfoEntry {
                label: "OS".into(),
                value: name,
            });
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 2: Memory
    {
        let mut s = Vec::new();
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;
            let mut cached = 0u64;
            for line in meminfo.lines() {
                if let Some(val) = line.strip_prefix("MemTotal:") {
                    total = parse_kb(val);
                } else if let Some(val) = line.strip_prefix("MemAvailable:") {
                    available = parse_kb(val);
                } else if let Some(val) = line.strip_prefix("Cached:")
                    && cached == 0
                {
                    cached = parse_kb(val);
                }
            }
            if total > 0 {
                let used = total.saturating_sub(available);
                s.push(SysInfoEntry {
                    label: "Total".into(),
                    value: format!("{:.1} GB", total as f64 / 1048576.0),
                });
                s.push(SysInfoEntry {
                    label: "Used".into(),
                    value: format!("{} MB", used / 1024),
                });
                s.push(SysInfoEntry {
                    label: "Cached".into(),
                    value: format!("{} MB", cached / 1024),
                });
            }
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 3: CPU
    {
        let mut s = Vec::new();
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            let cores = cpuinfo.matches("processor").count();
            let model = cpuinfo
                .lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|v| v.trim().to_string())
                .unwrap_or_default();
            if !model.is_empty() {
                s.push(SysInfoEntry {
                    label: "CPU".into(),
                    value: model,
                });
                s.push(SysInfoEntry {
                    label: "Cores".into(),
                    value: format!("{}", cores),
                });
            }
            if let Ok(stat) = std::fs::read_to_string("/proc/stat")
                && let Some(line) = stat.lines().find(|l| l.starts_with("cpu "))
            {
                let vals: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|v| v.parse().ok())
                    .collect();
                if vals.len() >= 4 {
                    let total: u64 = vals.iter().sum();
                    let idle = vals[3];
                    if total > 0 {
                        let usage = ((total - idle) as f64 / total as f64) * 100.0;
                        s.push(SysInfoEntry {
                            label: "Usage".into(),
                            value: format!("{:.1}%", usage),
                        });
                    }
                }
            }
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 4: Battery (if laptop)
    {
        let mut s = Vec::new();
        let bat_path = std::path::Path::new("/sys/class/power_supply/BAT0");
        if bat_path.exists() {
            if let Ok(cap) = std::fs::read_to_string(bat_path.join("capacity")) {
                s.push(SysInfoEntry {
                    label: "Charge".into(),
                    value: format!("{}%", cap.trim()),
                });
            }
            if let Ok(status) = std::fs::read_to_string(bat_path.join("status")) {
                s.push(SysInfoEntry {
                    label: "Status".into(),
                    value: status.trim().to_string(),
                });
            }
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 5: Uptime
    {
        let mut s = Vec::new();
        if let Ok(uptime) = std::fs::read_to_string("/proc/uptime")
            && let Some(secs_str) = uptime.split_whitespace().next()
            && let Ok(secs) = secs_str.parse::<f64>()
        {
            let total_secs = secs as u64;
            let days = total_secs / 86400;
            let hours = (total_secs % 86400) / 3600;
            let mins = (total_secs % 3600) / 60;
            let val = if days > 0 {
                format!("{}d {}h {}m", days, hours, mins)
            } else {
                format!("{}h {}m", hours, mins)
            };
            s.push(SysInfoEntry {
                label: "Time".into(),
                value: val,
            });
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    // Section 6: Disk
    {
        let mut s = Vec::new();
        if let Ok(output) = super::host_command("df").args(["-h", "/"]).output() {
            let out = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = out.lines().nth(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let device = parts[0].rsplit('/').next().unwrap_or(parts[0]);
                    s.push(SysInfoEntry {
                        label: device.to_string(),
                        value: format!("{} / {} ({})", parts[2], parts[1], parts[4]),
                    });
                }
            }
        }
        if !s.is_empty() {
            sections.push(s);
        }
    }

    sections
}

fn parse_kb(s: &str) -> u64 {
    s.split_whitespace()
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}
