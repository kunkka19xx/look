use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

const RELEASE_DOWNLOAD_URL: &str = "https://github.com/kunkka19xx/look/releases/download";
const DOWNLOAD_SCRIPT: &str = include_str!("update/download.ps1");
const INSTALL_SCRIPT: &str = include_str!("update/install.ps1");
const DOWNLOAD_SCRIPT_NAME: &str = "download.ps1";
const INSTALL_SCRIPT_NAME: &str = "install.ps1";
const ERROR_LOG_NAME: &str = "update-error.log";

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

/// Downloads and verifies the installer while Look keeps running, so a failed
/// download only reports an error. Look exits only once a verified installer
/// is on disk; the install helper then reopens it whether or not setup succeeds.
pub async fn start(app: AppHandle, version: String) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Self-update is disabled in dev builds".into());
    }
    if !is_valid_release_version(&version) {
        return Err(format!("Invalid release version: {version}"));
    }
    if detect_install_method() != InstallMethod::Nsis {
        return Err("Only installer builds can update themselves".into());
    }

    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let install_dir = exe
        .parent()
        .ok_or_else(|| format!("Missing install directory for {}", exe.display()))?
        .to_path_buf();
    let pid = std::process::id();
    let work_dir = helper_work_dir(pid)?;
    let setup_path = work_dir.join(setup_file_name(&version));

    let downloaded = {
        let (work_dir, setup_path) = (work_dir.clone(), setup_path.clone());
        tauri::async_runtime::spawn_blocking(move || download(&version, &work_dir, &setup_path))
            .await
            .unwrap_or_else(|e| Err(e.to_string()))
    };
    if let Err(e) = downloaded {
        let _ = fs::remove_dir_all(&work_dir);
        return Err(e);
    }

    let script = write_script(&work_dir, INSTALL_SCRIPT_NAME, INSTALL_SCRIPT)?;
    script_command(&script)
        .arg("-LookPid")
        .arg(pid.to_string())
        .arg("-SetupPath")
        .arg(&setup_path)
        .arg("-InstallDir")
        .arg(&install_dir)
        .arg("-ExePath")
        .arg(&exe)
        .arg("-LogPath")
        .arg(work_dir.join(ERROR_LOG_NAME))
        .spawn()
        .map_err(|e| format!("Failed to start the installer: {e}"))?;
    app.exit(0);
    Ok(())
}

fn download(version: &str, work_dir: &Path, setup_path: &Path) -> Result<(), String> {
    let script = write_script(work_dir, DOWNLOAD_SCRIPT_NAME, DOWNLOAD_SCRIPT)?;
    let base_url = format!("{RELEASE_DOWNLOAD_URL}/v{version}");
    let output = script_command(&script)
        .arg("-SetupUrl")
        .arg(format!("{base_url}/{}", setup_file_name(version)))
        .arg("-ChecksumsUrl")
        .arg(format!("{base_url}/Look-{version}-windows-checksums.txt"))
        .arg("-SetupPath")
        .arg(setup_path)
        .output()
        .map_err(|e| format!("Failed to start the download: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    Err(format!("Couldn't download the update: {}", reason.trim()))
}

fn setup_file_name(version: &str) -> String {
    format!("Look_{version}_x64-setup.exe")
}

fn detect_install_method_for_path(path: &Path) -> InstallMethod {
    let lower = path.to_string_lossy().replace('/', "\\").to_lowercase();
    if lower.contains("\\scoop\\apps\\look\\") {
        return InstallMethod::Scoop;
    }
    // Tauri's currentUser bundle defaults to %LOCALAPPDATA%\Look, while the
    // install script passes %LOCALAPPDATA%\Programs\Look via NSIS /D.
    if lower.contains("\\appdata\\local\\look\\")
        || lower.contains("\\appdata\\local\\programs\\look\\")
    {
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

fn write_script(work_dir: &Path, name: &str, contents: &str) -> Result<PathBuf, String> {
    let path = work_dir.join(name);
    fs::write(&path, contents).map_err(|e| format!("Failed to write {name}: {e}"))?;
    Ok(path)
}

fn script_command(script: &Path) -> Command {
    let mut cmd = Command::new("powershell.exe");
    // When launched from pwsh, Windows PowerShell can inherit PS7 module paths
    // and fail to load built-ins such as Get-FileHash.
    cmd.env_remove("PSModulePath")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .creation_flags(crate::consts::CREATE_NO_WINDOW);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Output;

    const TEST_VERSION: &str = "0.6.11";
    const INSTALLER_FAILURE_EXIT_CODE: i32 = 23;

    // Only the network and process boundaries are stubbed; the shipped scripts
    // run in Windows PowerShell (not pwsh) from a non-ASCII directory.
    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(case: &str) -> Self {
            let root = std::env::temp_dir().join(format!("look-update-test Nguyễn 更新 {case}"));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.root.join(name)
        }

        fn run(&self, script_name: &str, script: &str, prelude: &str, invocation: &str) -> Output {
            write_script(&self.root, script_name, script).unwrap();
            let wrapper = format!(
                "{prelude}\n\
                 function Log($Name) {{ Add-Content (Join-Path $PSScriptRoot 'events') $Name }}\n\
                 $Setup = Join-Path $PSScriptRoot '{setup}'\n\
                 {invocation}\n\
                 exit $LASTEXITCODE\n",
                setup = setup_file_name(TEST_VERSION),
            );
            let wrapper_path = write_script(&self.root, "test.ps1", &wrapper).unwrap();
            script_command(&wrapper_path).output().unwrap()
        }

        fn events(&self) -> Vec<String> {
            fs::read_to_string(self.path("events"))
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn run_download(fixture: &Fixture, checksum_line: &str) -> Output {
        let prelude = format!(
            r#"function Invoke-WebRequest {{
    param($Uri, $OutFile, [switch]$UseBasicParsing, $TimeoutSec)
    if ($Uri -like '*.exe') {{ [IO.File]::WriteAllText($OutFile, 'new installer'); return }}
    $Hash = (Get-FileHash $Setup -Algorithm SHA256).Hash
    "{checksum_line}" | Set-Content -Encoding ascii $OutFile
}}"#
        );
        fixture.run(
            DOWNLOAD_SCRIPT_NAME,
            DOWNLOAD_SCRIPT,
            &prelude,
            "& (Join-Path $PSScriptRoot 'download.ps1') -SetupUrl 'https://example.test/setup.exe' \
             -ChecksumsUrl 'https://example.test/checksums.txt' -SetupPath $Setup",
        )
    }

    fn run_install(fixture: &Fixture, installer_exit_code: i32) -> Output {
        fs::write(
            fixture.path(&setup_file_name(TEST_VERSION)),
            "new installer",
        )
        .unwrap();
        fs::write(fixture.path("lookapp.exe"), "old app").unwrap();
        let prelude = format!(
            r#"$InstallerExitCode = {installer_exit_code}
function Wait-Process {{ param($Id, $ErrorAction) Log 'wait' }}
function Start-Sleep {{ param($Milliseconds) }}
function Start-Process {{
    param($FilePath, $ArgumentList, [switch]$PassThru, [switch]$Wait)
    if ($FilePath -eq $Setup) {{
        if ($ArgumentList[0] -ne '/S' -or $ArgumentList[1] -ne "/D=$PSScriptRoot") {{ throw "Wrong installer arguments: $ArgumentList" }}
        Log 'install'
        return [pscustomobject]@{{ ExitCode = $InstallerExitCode }}
    }}
    if ($FilePath -eq 'notepad.exe') {{ Log 'error' }} else {{ Log 'restart' }}
}}"#
        );
        fixture.run(
            INSTALL_SCRIPT_NAME,
            INSTALL_SCRIPT,
            &prelude,
            "& (Join-Path $PSScriptRoot 'install.ps1') -LookPid 123 -SetupPath $Setup \
             -InstallDir $PSScriptRoot -ExePath (Join-Path $PSScriptRoot 'lookapp.exe') \
             -LogPath (Join-Path $PSScriptRoot 'update-error.log')",
        )
    }

    fn valid_checksum_line() -> String {
        format!("$Hash  {}", setup_file_name(TEST_VERSION))
    }

    #[test]
    fn download_keeps_a_verified_installer() {
        let fixture = Fixture::new("download ok");
        let output = run_download(&fixture, &valid_checksum_line());
        assert!(output.status.success(), "{output:?}");
        assert!(fixture.path(&setup_file_name(TEST_VERSION)).exists());
        let leftovers = fs::read_dir(&fixture.root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".checksums.txt")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn download_rejects_checksum_mismatch() {
        let fixture = Fixture::new("download mismatch");
        let bad_hash = "0".repeat(64);
        let output = run_download(
            &fixture,
            &format!("{bad_hash}  {}", setup_file_name(TEST_VERSION)),
        );
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("SHA256 mismatch"));
        assert!(!fixture.path(&setup_file_name(TEST_VERSION)).exists());
    }

    #[test]
    fn download_rejects_missing_checksum_entry() {
        let fixture = Fixture::new("download missing entry");
        let output = run_download(&fixture, "$Hash  Look_other_x64-setup.exe");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("no entry"));
        assert!(!fixture.path(&setup_file_name(TEST_VERSION)).exists());
    }

    #[test]
    fn install_runs_setup_then_restarts_look() {
        let fixture = Fixture::new("install ok");
        let output = run_install(&fixture, 0);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(fixture.events(), ["wait", "install", "restart"]);
        assert!(!fixture.path(&setup_file_name(TEST_VERSION)).exists());
        assert!(!fixture.path(ERROR_LOG_NAME).exists());
    }

    #[test]
    fn install_failure_still_restarts_look_and_shows_the_log() {
        let fixture = Fixture::new("install failed");
        let output = run_install(&fixture, INSTALLER_FAILURE_EXIT_CODE);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(fixture.events(), ["wait", "install", "restart", "error"]);
        let log = fs::read_to_string(fixture.path(ERROR_LOG_NAME)).unwrap();
        assert!(log.contains(&format!(
            "Installer exited with code {INSTALLER_FAILURE_EXIT_CODE}"
        )));
    }

    #[test]
    fn classifies_install_paths() {
        for (path, expected) in [
            (
                r"C:\Users\me\scoop\apps\look\0.6.11\lookapp.exe",
                InstallMethod::Scoop,
            ),
            (
                r"C:\Users\me\AppData\Local\Look\lookapp.exe",
                InstallMethod::Nsis,
            ),
            (
                r"C:\Users\me\AppData\Local\Programs\Look\lookapp.exe",
                InstallMethod::Nsis,
            ),
            (r"C:\other\lookapp.exe", InstallMethod::Unknown),
        ] {
            assert_eq!(
                detect_install_method_for_path(Path::new(path)),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn rejects_invalid_versions() {
        assert!(is_valid_release_version(TEST_VERSION));
        assert!(!is_valid_release_version("0.6.11; rm -rf"));
        assert!(!is_valid_release_version(""));
    }
}
