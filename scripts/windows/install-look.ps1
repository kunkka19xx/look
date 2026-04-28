<#
.SYNOPSIS
  Installs Look (Windows) from a GitHub release zip into a per-user location.

.DESCRIPTION
  Mirrors scripts/install-look.sh on macOS. Downloads the published release zip
  from kunkka19xx/look, verifies SHA256 against the release manifest, extracts
  to %LOCALAPPDATA%\Programs\Look, and drops Start menu + desktop shortcuts.
  Per-user install — no admin required, no PATH mutation.

  Targets Windows PowerShell 5.1 (default on Win10/11), no PS7-only syntax.

.PARAMETER Version
  Pin a specific version (e.g. "1.0.0"). When omitted, queries the GitHub API
  for the latest release.

.PARAMETER Repo
  Override the source repo (e.g. for forks). Default: kunkka19xx/look.

.PARAMETER Url
  Direct URL to a zip artifact, bypassing version resolution. Skips SHA256
  verification because there's no associated manifest file.

.PARAMETER InstallDir
  Custom install directory. Default: %LOCALAPPDATA%\Programs\Look.

.PARAMETER Launch
  Launch LauncherApp.exe after install. Default: $true. Pass -Launch:$false
  to skip.

.PARAMETER Uninstall
  Stop the app, remove the install directory, and delete shortcuts.

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
$ExeName = "LauncherApp.exe"
$ProcessName = "LauncherApp"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$StartMenuShortcut = Join-Path $StartMenuDir "$AppName.lnk"
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "$AppName.lnk"

function Write-Step($msg) {
    Write-Host "==> $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "    $msg" -ForegroundColor Green
}

function Write-Warn($msg) {
    Write-Host "    $msg" -ForegroundColor Yellow
}

function Detect-Arch {
    # PROCESSOR_ARCHITECTURE on x64 host running 32-bit PowerShell would be x86,
    # so prefer PROCESSOR_ARCHITEW6432 when present (set under WoW64).
    $arch = $env:PROCESSOR_ARCHITEW6432
    if ([string]::IsNullOrWhiteSpace($arch)) {
        $arch = $env:PROCESSOR_ARCHITECTURE
    }
    switch -Regex ($arch) {
        "ARM64" { return "arm64" }
        "AMD64" { return "x64" }
        default { throw "Unsupported architecture: $arch (only x64 and ARM64 are released)" }
    }
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

function Verify-Sha256($filePath, $manifestPath) {
    # All three failure modes (missing file, empty manifest, hash mismatch) throw —
    # a release path that calls into here has implicitly opted into integrity
    # checking, so falling back to "warn and continue" would defeat the point.
    if (-not (Test-Path $manifestPath)) {
        throw "Manifest file not present at $manifestPath; cannot verify zip integrity."
    }
    $expected = $null
    foreach ($line in Get-Content $manifestPath) {
        if ($line -match "^sha256=(.+)$") {
            $expected = $matches[1].Trim().ToLower()
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($expected)) {
        throw "Manifest at $manifestPath has no 'sha256=' line; cannot verify zip integrity."
    }
    $actual = (Get-FileHash -Path $filePath -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) {
        throw "SHA256 mismatch. expected=$expected actual=$actual"
    }
    Write-Ok "SHA256 verified."
}

function New-Shortcut($shortcutPath, $targetExe, $description) {
    $dir = Split-Path -Parent $shortcutPath
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $shell = New-Object -ComObject WScript.Shell
    $sc = $shell.CreateShortcut($shortcutPath)
    $sc.TargetPath = $targetExe
    $sc.WorkingDirectory = Split-Path -Parent $targetExe
    $sc.Description = $description
    $sc.IconLocation = "$targetExe,0"
    $sc.Save()
}

function Invoke-Install {
    if (-not [string]::IsNullOrWhiteSpace($Url)) {
        $zipUrl = $Url
        $manifestUrl = $null
        $resolvedVersion = "(custom)"
    } else {
        if ([string]::IsNullOrWhiteSpace($Version)) {
            $Version = Resolve-LatestVersion $Repo
        }
        $resolvedVersion = $Version
        $arch = Detect-Arch
        Write-Step "Architecture: $arch"
        $base = "https://github.com/$Repo/releases/download/v$Version"
        $zipUrl = "$base/Look-$Version-windows-$arch.zip"
        $manifestUrl = "$base/Look-$Version-windows-$arch-manifest.txt"
    }

    $tmp = Join-Path $env:TEMP "look-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    try {
        $zipPath = Join-Path $tmp "look.zip"
        Download-File $zipUrl $zipPath

        if ($manifestUrl) {
            # For the normal release path (resolved version + GitHub release), the
            # manifest is the only integrity check we have on the zip we just
            # downloaded. Silently skipping verification on download/parse failure
            # would let a blocked or tampered manifest pass through and install an
            # unverified zip. Fail closed instead. The -Url custom path explicitly
            # sets $manifestUrl to $null and bypasses this branch when the user
            # has accepted that they're installing without checksum coverage.
            $manifestPath = Join-Path $tmp "look-manifest.txt"
            Download-File $manifestUrl $manifestPath
            Verify-Sha256 $zipPath $manifestPath
        }

        Stop-LookProcess

        if (Test-Path $InstallDir) {
            Write-Step "Removing previous install at $InstallDir"
            Remove-Item -Path $InstallDir -Recurse -Force
        }

        Write-Step "Extracting to $InstallDir"
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Expand-Archive -Path $zipPath -DestinationPath $InstallDir -Force

        $exePath = Join-Path $InstallDir $ExeName
        if (-not (Test-Path $exePath)) {
            throw "Extraction did not produce $ExeName at $exePath"
        }

        Write-Step "Creating shortcuts"
        New-Shortcut $StartMenuShortcut $exePath "Keyboard-first launcher"
        New-Shortcut $DesktopShortcut $exePath "Keyboard-first launcher"
        Write-Ok "Start menu: $StartMenuShortcut"
        Write-Ok "Desktop:    $DesktopShortcut"

        Write-Host ""
        Write-Host "Look $resolvedVersion installed to $InstallDir" -ForegroundColor Green
        Write-Host "  - Press Alt+Space to summon (configurable in Settings -> Appearance)" -ForegroundColor Green
        Write-Host "  - On first run, Windows SmartScreen may show a warning." -ForegroundColor Green
        Write-Host "    Click 'More info' -> 'Run anyway'. Reputation builds with installs." -ForegroundColor Green
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

    if (Test-Path $InstallDir) {
        Write-Step "Removing $InstallDir"
        Remove-Item -Path $InstallDir -Recurse -Force
    } else {
        Write-Warn "Install directory not found: $InstallDir"
    }

    foreach ($sc in @($StartMenuShortcut, $DesktopShortcut)) {
        if (Test-Path $sc) {
            Remove-Item -Path $sc -Force
            Write-Ok "Removed $sc"
        }
    }

    Write-Host ""
    Write-Host "Look uninstalled." -ForegroundColor Green
}

if ($Uninstall) {
    Invoke-Uninstall
} else {
    Invoke-Install
}
