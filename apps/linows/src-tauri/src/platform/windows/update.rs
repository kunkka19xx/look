use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
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
            let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
            let restart_exe = scoop_restart_path(&exe)
                .ok_or("Cannot locate Scoop installation. Check your Scoop.")?;
            let work_dir = helper_work_dir(std::process::id())?;
            let script_path = work_dir.join("apply-look-update.ps1");
            fs::write(
                &script_path,
                build_scoop_helper_script(std::process::id(), &restart_exe, &work_dir),
            )
            .map_err(|e| format!("Failed to write Scoop update script: {e}. Check your Scoop."))?;
            spawn_helper(&script_path).map_err(|e| format!("{e}. Check your Scoop."))?;
            app.exit(0);
            Ok(())
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
    if lower.contains("\\appdata\\local\\look\\") {
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

fn powershell_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("powershell.exe");
    // When launched via cargo/app from pwsh, Windows PowerShell can inherit
    // PS7 module paths and fail to load built-ins such as Get-FileHash.
    // Let Windows PowerShell reconstruct its own module search path.
    cmd.env_remove("PSModulePath");
    cmd
}

fn spawn_helper(script_path: &Path) -> Result<(), String> {
    let mut cmd = powershell_command();
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script_path)
        // A visible shell shows update progress without requiring users to type commands.
        .creation_flags(crate::consts::CREATE_WINDOW_CONSOLE);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to spawn update helper: {e}"))
}

fn scoop_restart_path(exe: &Path) -> Option<PathBuf> {
    // current_exe may resolve Scoop's current junction to a version directory.
    // Restart through current so it points to the version Scoop just installed.
    let package = exe.ancestors().find(|path| {
        path.file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("look"))
            && path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("apps"))
    })?;
    let relative = exe.strip_prefix(package).ok()?;
    Some(
        package
            .join("current")
            .join(relative.components().skip(1).collect::<PathBuf>()),
    )
}

fn build_scoop_helper_script(current_pid: u32, exe_path: &Path, work_dir: &Path) -> String {
    let exe_path = ps_single_quoted(&exe_path.to_string_lossy());
    let work_dir = ps_single_quoted(&work_dir.to_string_lossy());
    format!(
        "\u{feff}$ErrorActionPreference = 'Stop'\n\
         $ExePath = '{exe_path}'\n\
         $LogPath = Join-Path '{work_dir}' 'update-error.log'\n\
         try {{\n\
           Wait-Process -Id {current_pid} -ErrorAction SilentlyContinue\n\
           Start-Sleep -Milliseconds 350\n\
           $OldHash = (Get-FileHash -Path $ExePath -Algorithm SHA256).Hash\n\
           $proc = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', \"scoop update look; if (-not `$?) {{ exit 1 }}\") -NoNewWindow -PassThru -Wait\n\
           if ($proc.ExitCode -ne 0) {{ throw \"scoop update look exited with code $($proc.ExitCode)\" }}\n\
           if (-not (Test-Path $ExePath)) {{ throw 'Updated Look executable not found' }}\n\
           if ((Get-FileHash -Path $ExePath -Algorithm SHA256).Hash -eq $OldHash) {{ throw 'Scoop did not install a newer Look version' }}\n\
           Start-Process -FilePath $ExePath | Out-Null\n\
         }} catch {{\n\
           \"Scoop update failed. Check your Scoop. $($_.Exception.Message)\" | Out-File -FilePath $LogPath -Encoding utf8\n\
           Start-Process -FilePath 'notepad.exe' -ArgumentList $LogPath | Out-Null\n\
         }}\n"
    )
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
    // Windows PowerShell needs a UTF-8 BOM to decode non-ASCII paths correctly.
    let mut script = String::from("\u{feff}");
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
    script.push_str("  Invoke-WebRequest -Uri \"$BaseUrl/$SetupFileName\" -OutFile $SetupPath -UseBasicParsing -TimeoutSec 300\n");
    script.push_str("  Invoke-WebRequest -Uri \"$BaseUrl/$ChecksumsFileName\" -OutFile $ChecksumsPath -UseBasicParsing -TimeoutSec 60\n");
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
    use super::{
        InstallMethod, build_helper_script, detect_install_method_for_path,
        is_valid_release_version,
    };
    use std::path::Path;

    #[test]
    fn helper_script_has_utf8_bom_and_preserves_unicode_paths() {
        let script = build_helper_script(
            "0.6.11",
            123,
            Path::new(r"C:\Users\Nguyễn\AppData\Local\Programs\Look"),
            Path::new(r"C:\Users\Nguyễn\AppData\Local\Programs\Look\lookapp.exe"),
            Path::new(r"C:\Users\Nguyễn\AppData\Local\Temp\更新"),
        );

        let bytes = script.as_bytes();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let decoded = std::str::from_utf8(&bytes[3..]).unwrap();
        assert!(decoded.starts_with("$ErrorActionPreference = 'Stop'\n"));
        for expected in [
            r"$InstallDir = 'C:\Users\Nguyễn\AppData\Local\Programs\Look'",
            r"$ExePath = 'C:\Users\Nguyễn\AppData\Local\Programs\Look\lookapp.exe'",
            r"$WorkDir = 'C:\Users\Nguyễn\AppData\Local\Temp\更新'",
        ] {
            assert!(decoded.lines().any(|line| line == expected), "{expected}");
        }
    }

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
                r"C:\Users\me\AppData\Local\Look\lookapp.exe"
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

    // Execute the generated helper in Windows PowerShell (not pwsh). Only the
    // network and process boundaries are stubbed; checksum, control flow,
    // error handling, BOM decoding and cleanup run as shipped.
    #[test]
    fn helper_executes_successful_update() {
        run_helper_case(0, false, false, false, None);
    }

    #[test]
    fn helper_rejects_checksum_mismatch_without_installing() {
        run_helper_case(0, true, false, false, Some("SHA256 mismatch"));
    }

    #[test]
    fn helper_reports_installer_failure_without_restarting() {
        run_helper_case(
            23,
            false,
            false,
            false,
            Some("Installer exited with code 23"),
        );
    }

    #[test]
    fn scoop_helper_updates_and_restarts() {
        run_helper_case(0, false, true, false, None);
    }

    #[test]
    fn scoop_helper_reports_failure_without_restarting() {
        run_helper_case(1, false, true, false, Some("Check your Scoop"));
    }

    #[test]
    fn scoop_restarts_through_current_instead_of_the_old_version() {
        assert_eq!(
            super::scoop_restart_path(Path::new(
                r"C:\Users\Nguyễn\scoop\apps\look\0.6.11\lookapp.exe"
            )),
            Some(Path::new(r"C:\Users\Nguyễn\scoop\apps\look\current\lookapp.exe").to_path_buf())
        );
        assert!(super::scoop_restart_path(Path::new(r"C:\other\lookapp.exe")).is_none());
    }

    #[test]
    fn scoop_no_change_does_not_restart_into_an_auto_update_loop() {
        run_helper_case(
            0,
            false,
            true,
            true,
            Some("Scoop did not install a newer Look version"),
        );
    }

    fn run_helper_case(
        exit_code: i32,
        bad_checksum: bool,
        scoop: bool,
        no_change: bool,
        expected_error: Option<&str>,
    ) {
        use std::fs;

        let root = super::helper_work_dir(std::process::id())
            .unwrap()
            .join(format!(
                "Nguyễn 更新 {exit_code}-{bad_checksum}-{scoop}-{no_change}"
            ));
        fs::create_dir_all(&root).unwrap();
        let exe = root.join("lookapp.exe");
        fs::write(&exe, "old app").unwrap();
        let helper = root.join("helper.ps1");
        fs::write(
            &helper,
            if scoop {
                super::build_scoop_helper_script(123, &exe, &root)
            } else {
                build_helper_script("0.6.11", 123, &root, &exe, &root)
            },
        )
        .unwrap();
        let wrapper = root.join("test.ps1");
        // PowerShell can expand TEMP's 8.3 aliases in $PSScriptRoot, whereas
        // Rust keeps the supplied path (e.g. RUNNER~1). Assert against the
        // fixture paths passed to the helper, not PowerShell's spelling.
        let prelude = format!(
            "$InstallerExitCode = {exit_code}\n$BadChecksum = ${bad_checksum}\n$NoChange = ${no_change}\n\
             $ExpectedInstallDir = '{}'\n$ExpectedExePath = '{}'\n",
            super::ps_single_quoted(&root.to_string_lossy()),
            super::ps_single_quoted(&exe.to_string_lossy()),
        );
        let stubs = r#"
$ErrorActionPreference = 'Stop'
function Wait-Process { param($Id, $ErrorAction) Add-Content (Join-Path $PSScriptRoot 'events') 'wait' }
function Start-Sleep { param($Milliseconds) }
function Invoke-WebRequest {
    param($Uri, $OutFile, [switch]$UseBasicParsing, $TimeoutSec)
    Add-Content (Join-Path $PSScriptRoot 'events') 'download'
    if ($OutFile.EndsWith('.exe')) {
        [IO.File]::WriteAllText($OutFile, 'new installer')
    } else {
        $hash = (Get-FileHash (Join-Path $PSScriptRoot 'Look_0.6.11_x64-setup.exe')).Hash
        if ($BadChecksum) { $hash = '0' * 64 }
        "$hash  Look_0.6.11_x64-setup.exe" | Set-Content -Encoding ascii $OutFile
    }
}
function Start-Process {
    param($FilePath, $ArgumentList, [switch]$PassThru, [switch]$Wait, [switch]$NoNewWindow)
    if ($FilePath -eq 'powershell.exe') {
        if (-not $Wait -or -not $PassThru -or -not $NoNewWindow) { throw 'Scoop must be awaited in the visible shell' }
        if ($ArgumentList[4] -ne 'scoop update look; if (-not $?) { exit 1 }') { throw 'Wrong Scoop command' }
        Add-Content (Join-Path $PSScriptRoot 'events') 'scoop'
        if ($InstallerExitCode -eq 0 -and -not $NoChange) { Set-Content (Join-Path $PSScriptRoot 'lookapp.exe') 'new app' }
        return [pscustomobject]@{ ExitCode = $InstallerExitCode }
    }
    if ($FilePath.EndsWith('-setup.exe')) {
        if (-not $Wait -or -not $PassThru) { throw 'Installer must be awaited' }
        if ($ArgumentList.Count -ne 2 -or $ArgumentList[0] -ne '/S' -or $ArgumentList[1] -ne "/D=$ExpectedInstallDir") {
            throw "Wrong installer arguments: $($ArgumentList -join ' | '); expected /S | /D=$ExpectedInstallDir"
        }
        Add-Content (Join-Path $PSScriptRoot 'events') 'install'
        return [pscustomobject]@{ ExitCode = $InstallerExitCode }
    }
    if ($FilePath -eq 'notepad.exe') {
        Add-Content (Join-Path $PSScriptRoot 'events') 'error'
    } elseif ($FilePath -eq $ExpectedExePath) {
        Add-Content (Join-Path $PSScriptRoot 'events') 'restart'
    } else { throw "Unexpected process: $FilePath" }
}
. (Join-Path $PSScriptRoot 'helper.ps1')
"#;
        fs::write(&wrapper, format!("\u{feff}{prelude}{stubs}")).unwrap();
        let output = super::powershell_command()
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&wrapper)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let events = fs::read_to_string(root.join("events")).unwrap();
        let events: Vec<_> = events.lines().collect();
        if let Some(error) = expected_error {
            let log = fs::read_to_string(root.join("update-error.log")).unwrap();
            assert!(log.contains(error), "{log}");
            assert!(!events.contains(&"restart"));
            assert_eq!(events.last(), Some(&"error"));
            if bad_checksum {
                assert!(!events.contains(&"install"));
            }
        } else {
            if let Ok(log) = fs::read_to_string(root.join("update-error.log")) {
                panic!("Update helper failed unexpectedly:\n{log}");
            }
            if scoop {
                assert_eq!(events, ["wait", "scoop", "restart"]);
            } else {
                assert_eq!(
                    events,
                    ["wait", "download", "download", "install", "restart"]
                );
            }
            assert!(!root.join("Look_0.6.11_x64-setup.exe").exists());
            assert!(!root.join("Look-0.6.11-windows-checksums.txt").exists());
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
