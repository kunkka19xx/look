<#
.SYNOPSIS
  Builds a fake "installed" Look so the Windows self-update path can be tested.

.DESCRIPTION
  The Update button only renders when the app is a release build (is_dev_build
  false) living under %LOCALAPPDATA%\Programs\Look (detect_install_method ->
  nsis). This builds a release lookapp.exe stamped with an older version and
  drops it there, so "Check for Updates" finds the real GitHub release and the
  full download -> checksum -> install -> relaunch flow runs for real.

  The version is stamped through LOOK_VERSION, which backend/build.rs reads.

.PARAMETER Version
  Version to stamp on the fake build. Must be lower than the latest release.

.PARAMETER SkipBuild
  Reuse an existing release binary instead of rebuilding.

.PARAMETER Uninstall
  Remove the fake install directory.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\fake-install.ps1
#>

[CmdletBinding()]
param(
    [string]$Version = "0.6.12",
    [string]$InstallDir = "",
    [switch]$SkipBuild,
    [switch]$NoLaunch,
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$exePath = Join-Path $repoRoot "apps\linows\target\x86_64-pc-windows-msvc\release\lookapp.exe"
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\Look" }
$installedExe = Join-Path $InstallDir "lookapp.exe"

cmd /c "taskkill /F /IM lookapp.exe >nul 2>&1"

if ($Uninstall) {
    if (Test-Path $InstallDir) {
        Remove-Item -Recurse -Force $InstallDir
        Write-Host "Removed $InstallDir"
    } else {
        Write-Host "Nothing at $InstallDir"
    }
    return
}

if (-not $SkipBuild) {
    $env:LOOK_VERSION = $Version
    try {
        Write-Host "Building release lookapp.exe as $Version"
        & (Join-Path $repoRoot "scripts\windows\with-vcvars.bat") cargo build `
            --manifest-path (Join-Path $repoRoot "apps\linows\Cargo.toml") -p lookapp `
            --target x86_64-pc-windows-msvc --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
    } finally {
        Remove-Item Env:\LOOK_VERSION -ErrorAction SilentlyContinue
    }
}

if (-not (Test-Path $exePath)) { throw "Missing $exePath" }

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Force $exePath $installedExe
Write-Host "Installed $installedExe"

if (-not $NoLaunch) { Start-Process -FilePath $installedExe }
