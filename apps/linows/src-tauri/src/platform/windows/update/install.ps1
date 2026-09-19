# Runs after Look exits: installs the verified setup, then always reopens Look.
param(
    [Parameter(Mandatory)] [int] $LookPid,
    [Parameter(Mandatory)] [string] $SetupPath,
    [Parameter(Mandatory)] [string] $InstallDir,
    [Parameter(Mandatory)] [string] $ExePath,
    [Parameter(Mandatory)] [string] $LogPath
)
$ErrorActionPreference = 'Stop'
$FileLockReleaseDelayMs = 350
$failed = $false

try {
    Wait-Process -Id $LookPid -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds $FileLockReleaseDelayMs
    $proc = Start-Process -FilePath $SetupPath -ArgumentList @('/S', "/D=$InstallDir") -PassThru -Wait
    if ($proc.ExitCode -ne 0) { throw "Installer exited with code $($proc.ExitCode)" }
} catch {
    $failed = $true
    $_ | Out-File -FilePath $LogPath -Encoding utf8
}
Remove-Item -Path $SetupPath -Force -ErrorAction SilentlyContinue
if (Test-Path $ExePath) { Start-Process -FilePath $ExePath | Out-Null }
if ($failed) { Start-Process -FilePath 'notepad.exe' -ArgumentList $LogPath | Out-Null }
