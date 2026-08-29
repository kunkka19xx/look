use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const REPO: &str = "kunkka19xx/look";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMethod {
    Nsis,
    Scoop,
    Unknown,
}

impl InstallMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nsis => "nsis",
            Self::Scoop => "scoop",
            Self::Unknown => "unknown",
        }
    }
}

pub fn detect_install_method() -> InstallMethod {
    let Ok(exe) = std::env::current_exe() else {
        return InstallMethod::Unknown;
    };
    detect_install_method_for_path(&exe)
}

pub fn start(app: AppHandle, version: &str) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Self-update is disabled in dev builds".into());
    }
    if !is_valid_release_version(version) {
        return Err(format!("Invalid release version: {version}"));
    }

    match detect_install_method() {
        InstallMethod::Nsis => {
            let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
            let install_dir = exe
                .parent()
                .ok_or_else(|| format!("Missing install directory for {}", exe.display()))?;
            let work_dir = helper_work_dir(std::process::id())?;
            let script_path = work_dir.join("apply-look-update.ps1");
            let script =
                build_helper_script(version, std::process::id(), install_dir, &exe, &work_dir);
            fs::write(&script_path, script)
                .map_err(|e| format!("Failed to write update helper script: {e}"))?;
            spawn_helper(&script_path)?;
            app.exit(0);
            Ok(())
        }
        InstallMethod::Scoop => {
            Err("Look was installed with Scoop. Run 'scoop update look'.".into())
        }
        InstallMethod::Unknown => {
            Err("Unknown install method. Please update Look from the release page.".into())
        }
    }
}

fn detect_install_method_for_path(path: &Path) -> InstallMethod {
    let lower = path.to_string_lossy().replace('/', "\\").to_lowercase();
    if lower.contains("\\scoop\\apps\\look\\") {
        return InstallMethod::Scoop;
    }
    if lower.contains("\\appdata\\local\\programs\\look\\") {
        return InstallMethod::Nsis;
    }
    InstallMethod::Unknown
}

fn is_valid_release_version(version: &str) -> bool {
    let trimmed = version.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn helper_work_dir(pid: u32) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("look-update-{pid}-{stamp}"));
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create updater temp dir: {e}"))?;
    Ok(dir)
}

fn spawn_helper(script_path: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-WindowStyle",
        "Hidden",
        "-File",
    ])
    .arg(script_path)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .creation_flags(crate::consts::CREATE_NO_WINDOW);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to spawn update helper: {e}"))
}

fn build_helper_script(
    version: &str,
    current_pid: u32,
    install_dir: &Path,
    exe_path: &Path,
    work_dir: &Path,
) -> String {
    let version = ps_single_quoted(version);
    let install_dir = ps_single_quoted(&install_dir.to_string_lossy());
    let exe_path = ps_single_quoted(&exe_path.to_string_lossy());
    let work_dir = ps_single_quoted(&work_dir.to_string_lossy());
    let repo = ps_single_quoted(REPO);
    let mut script = String::new();
    script.push_str("$ErrorActionPreference = 'Stop'\n");
    script.push_str("$ProgressPreference = 'SilentlyContinue'\n");
    let _ = writeln!(script, "$Version = '{version}'");
    let _ = writeln!(script, "$CurrentPid = {current_pid}");
    let _ = writeln!(script, "$InstallDir = '{install_dir}'");
    let _ = writeln!(script, "$ExePath = '{exe_path}'");
    let _ = writeln!(script, "$WorkDir = '{work_dir}'");
    let _ = writeln!(script, "$Repo = '{repo}'");
    script.push_str("$SetupFileName = \"Look_${Version}_x64-setup.exe\"\n");
    script.push_str("$ChecksumsFileName = \"Look-$Version-windows-checksums.txt\"\n");
    script.push_str("$BaseUrl = \"https://github.com/$Repo/releases/download/v$Version\"\n");
    script.push_str("$SetupPath = Join-Path $WorkDir $SetupFileName\n");
    script.push_str("$ChecksumsPath = Join-Path $WorkDir $ChecksumsFileName\n");
    script.push_str("$LogPath = Join-Path $WorkDir 'update-error.log'\n");
    script.push_str("try {\n");
    script.push_str("  if ($CurrentPid -gt 0) {\n");
    script.push_str("    Wait-Process -Id $CurrentPid -ErrorAction SilentlyContinue\n");
    script.push_str("    Start-Sleep -Milliseconds 350\n");
    script.push_str("  }\n");
    script.push_str("  Invoke-WebRequest -Uri \"$BaseUrl/$SetupFileName\" -OutFile $SetupPath -UseBasicParsing\n");
    script.push_str("  Invoke-WebRequest -Uri \"$BaseUrl/$ChecksumsFileName\" -OutFile $ChecksumsPath -UseBasicParsing\n");
    script.push_str("  $expected = $null\n");
    script.push_str("  foreach ($line in Get-Content $ChecksumsPath) {\n");
    script.push_str("    $line = $line.Trim()\n");
    script.push_str("    if ([string]::IsNullOrWhiteSpace($line)) { continue }\n");
    script.push_str("    $parts = $line -split '\\s+', 2\n");
    script.push_str("    if ($parts.Count -ne 2) { continue }\n");
    script.push_str("    $name = $parts[1].Trim().TrimStart('*')\n");
    script.push_str("    if ($name -eq $SetupFileName) {\n");
    script.push_str("      $expected = $parts[0].Trim().ToLower()\n");
    script.push_str("      break\n");
    script.push_str("    }\n");
    script.push_str("  }\n");
    script.push_str("  if ([string]::IsNullOrWhiteSpace($expected)) {\n");
    script.push_str("    throw \"Checksums file has no entry for '$SetupFileName'.\"\n");
    script.push_str("  }\n");
    script
        .push_str("  $actual = (Get-FileHash -Path $SetupPath -Algorithm SHA256).Hash.ToLower()\n");
    script.push_str("  if ($actual -ne $expected) {\n");
    script.push_str("    throw \"SHA256 mismatch. expected=$expected actual=$actual\"\n");
    script.push_str("  }\n");
    script.push_str("  $proc = Start-Process -FilePath $SetupPath -ArgumentList @('/S', \"/D=$InstallDir\") -PassThru -Wait\n");
    script.push_str("  if ($proc.ExitCode -ne 0) {\n");
    script.push_str("    throw \"Installer exited with code $($proc.ExitCode)\"\n");
    script.push_str("  }\n");
    script.push_str("  if (Test-Path $ExePath) {\n");
    script.push_str("    Start-Process -FilePath $ExePath | Out-Null\n");
    script.push_str("  }\n");
    script.push_str(
        "  Remove-Item -Path $SetupPath, $ChecksumsPath -Force -ErrorAction SilentlyContinue\n",
    );
    script.push_str("} catch {\n");
    script.push_str("  $_ | Out-File -FilePath $LogPath -Encoding utf8\n");
    script.push_str("  Start-Process -FilePath 'notepad.exe' -ArgumentList $LogPath | Out-Null\n");
    script.push_str("}\n");
    script
}

fn ps_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::{InstallMethod, detect_install_method_for_path, is_valid_release_version};
    use std::path::Path;

    #[test]
    fn classifies_scoop_paths() {
        assert_eq!(
            detect_install_method_for_path(Path::new(
                r"C:\Users\me\scoop\apps\look\0.6.11\lookapp.exe"
            )),
            InstallMethod::Scoop
        );
    }

    #[test]
    fn classifies_nsis_paths() {
        assert_eq!(
            detect_install_method_for_path(Path::new(
                r"C:\Users\me\AppData\Local\Programs\Look\lookapp.exe"
            )),
            InstallMethod::Nsis
        );
    }

    #[test]
    fn rejects_invalid_versions() {
        assert!(is_valid_release_version("0.6.11"));
        assert!(!is_valid_release_version("0.6.11; rm -rf"));
        assert!(!is_valid_release_version(""));
    }
}
