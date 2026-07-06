<#
.SYNOPSIS
  Installs Look (Windows, Tauri) from a GitHub release via the NSIS installer.

.DESCRIPTION
  Resolves the latest release on kunkka19xx/look, downloads
  Look_<version>_x64-setup.exe, verifies its SHA256 against the published
  checksums file, and runs the installer silently. The installer drops a
  per-user install under %LOCALAPPDATA%\Programs\Look with Start menu entries.
  No admin rights required.

  Targets Windows PowerShell 5.1 (default on Win10/11). Avoids PS7-only syntax.

.PARAMETER Version
  Pin a specific version (e.g. "1.0.0"). When omitted, queries the GitHub API
  for the latest release.

.PARAMETER Repo
  Override the source repo (e.g. for forks). Default: kunkka19xx/look.

.PARAMETER Url
  Direct URL to a setup .exe, bypassing version resolution. Skips SHA256
  verification because there's no associated checksums file.

.PARAMETER InstallDir
  Custom install directory. Default: %LOCALAPPDATA%\Programs\Look.

.PARAMETER Launch
  Launch lookapp.exe after install. Default: $true. Pass -Launch:$false
  to skip.

.PARAMETER Uninstall
  Run the bundled NSIS uninstaller silently.

.EXAMPLE
  # Install latest from a fresh shell
  iex "& { $(irm https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/windows/install-look.ps1) }"

.EXAMPLE
  # Pin a version
  iex "& { $(irm https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/windows/install-look.ps1) } -Version 1.0.0"

.EXAMPLE
  # Uninstall
  iex "& { $(irm https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/windows/install-look.ps1) } -Uninstall"
#>

[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$Repo = "kunkka19xx/look",
    [string]$Url = "",
    [string]$InstallDir = "",
    [switch]$Launch = $true,
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"  # Invoke-WebRequest progress bar tanks throughput in PS 5.1

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\Look"
}

$AppName = "Look"
$ExeName = "lookapp.exe"
$ProcessName = "lookapp"

function Write-Step($msg) {
    Write-Host "==> $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "    $msg" -ForegroundColor Green
}

function Write-Warn($msg) {
    Write-Host "    $msg" -ForegroundColor Yellow
}

function Resolve-LatestVersion($repo) {
    $api = "https://api.github.com/repos/$repo/releases/latest"
    Write-Step "Resolving latest release from $api"
    try {
        $headers = @{ "User-Agent" = "look-installer" }
        $resp = Invoke-RestMethod -Uri $api -Headers $headers -Method Get
    } catch {
        throw "Failed to query GitHub API ($api): $_"
    }
    $tag = $resp.tag_name
    if ([string]::IsNullOrWhiteSpace($tag)) {
        throw "Latest release has no tag_name."
    }
    if ($tag.StartsWith("v")) {
        $tag = $tag.Substring(1)
    }
    Write-Ok "Latest version: $tag"
    return $tag
}

function Stop-LookProcess {
    $procs = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
    if ($procs) {
        Write-Step "Stopping running $ProcessName process(es)"
        $procs | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 300
    }
}

function Download-File($url, $dest) {
    Write-Step "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
    if (-not (Test-Path $dest)) {
        throw "Download failed: $dest"
    }
}

function Verify-Sha256($filePath, $checksumsPath, $expectedFileName) {
    # Failing closed: any of (missing checksums, no match for the filename, hash
    # mismatch) throws. The -Url path bypasses this entirely by passing $null.
    if (-not (Test-Path $checksumsPath)) {
        throw "Checksums file not present at $checksumsPath; cannot verify integrity."
    }
    $expected = $null
    foreach ($line in Get-Content $checksumsPath) {
        $line = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        # Standard `sha256sum` format: "<hash>  <filename>" (two spaces).
        $parts = $line -split '\s+', 2
        if ($parts.Count -ne 2) { continue }
        $name = $parts[1].Trim().TrimStart('*')
        if ($name -eq $expectedFileName) {
            $expected = $parts[0].Trim().ToLower()
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($expected)) {
        throw "Checksums file has no entry for '$expectedFileName'; cannot verify."
    }
    $actual = (Get-FileHash -Path $filePath -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) {
        throw "SHA256 mismatch. expected=$expected actual=$actual"
    }
    Write-Ok "SHA256 verified."
}

function Invoke-Install {
    if (-not [string]::IsNullOrWhiteSpace($Url)) {
        $setupUrl = $Url
        $checksumsUrl = $null
        $resolvedVersion = "(custom)"
        $setupFileName = Split-Path -Leaf $Url
    } else {
        if ([string]::IsNullOrWhiteSpace($Version)) {
            $Version = Resolve-LatestVersion $Repo
        }
        $resolvedVersion = $Version
        # NSIS installer naming from tauri-cli is `<ProductName>_<version>_x64-setup.exe`.
        $setupFileName = "Look_${Version}_x64-setup.exe"
        $base = "https://github.com/$Repo/releases/download/v$Version"
        $setupUrl = "$base/$setupFileName"
        $checksumsUrl = "$base/Look-$Version-windows-checksums.txt"
    }

    $tmp = Join-Path $env:TEMP "look-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    try {
        $setupPath = Join-Path $tmp $setupFileName
        Download-File $setupUrl $setupPath

        if ($checksumsUrl) {
            $checksumsPath = Join-Path $tmp "look-checksums.txt"
            Download-File $checksumsUrl $checksumsPath
            Verify-Sha256 $setupPath $checksumsPath $setupFileName
        }

        Stop-LookProcess

        Write-Step "Running NSIS installer silently"
        # /S = silent; /D=<dir> = install directory (must be the LAST argument
        # and unquoted per NSIS convention, even if the path contains spaces).
        $args = @("/S", "/D=$InstallDir")
        $proc = Start-Process -FilePath $setupPath -ArgumentList $args -PassThru -Wait
        if ($proc.ExitCode -ne 0) {
            throw "Installer exited with code $($proc.ExitCode)"
        }

        $exePath = Join-Path $InstallDir $ExeName
        if (-not (Test-Path $exePath)) {
            throw "Install completed but $ExeName not found at $exePath"
        }
        Write-Ok "Installed to $InstallDir"

        Write-Host ""
        Write-Host "Look $resolvedVersion installed." -ForegroundColor Green
        Write-Host "  - Press Alt+Space to summon (hotkey is fixed for now)" -ForegroundColor Green
        Write-Host "  - Uninstall later: rerun this script with -Uninstall" -ForegroundColor Green
        Write-Host ""

        if ($Launch) {
            Write-Step "Launching $ExeName"
            Start-Process -FilePath $exePath
        }
    } finally {
        Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-Uninstall {
    Stop-LookProcess

    # Tauri's NSIS template emits `uninstall.exe` at the install root.
    $uninstaller = Join-Path $InstallDir "uninstall.exe"
    if (Test-Path $uninstaller) {
        Write-Step "Running uninstaller: $uninstaller"
        $proc = Start-Process -FilePath $uninstaller -ArgumentList "/S" -PassThru -Wait
        if ($proc.ExitCode -ne 0) {
            Write-Warn "Uninstaller exited with code $($proc.ExitCode)"
        }
    } else {
        Write-Warn "No uninstall.exe at $InstallDir - was Look installed via NSIS?"
        if (Test-Path $InstallDir) {
            Write-Step "Removing $InstallDir manually"
            Remove-Item -Path $InstallDir -Recurse -Force
        }
    }

    Write-Host ""
    Write-Host "Look uninstalled." -ForegroundColor Green
    Write-Host "  - User data under %LOCALAPPDATA%\look is left in place." -ForegroundColor Green
    Write-Host "    Run: Remove-Item -Recurse `"`$env:LOCALAPPDATA\look`" to wipe it." -ForegroundColor Green
    Write-Host ""
}

if ($Uninstall) {
    Invoke-Uninstall
} else {
    Invoke-Install
}
