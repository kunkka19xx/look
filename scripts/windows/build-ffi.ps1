param(
    [Parameter(Mandatory = $true)]
    [string]$ManifestPath,
    [Parameter(Mandatory = $false)]
    [ValidateSet("x64", "arm64")]
    [string]$Arch = "x64",
    [switch]$Release
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ManifestPath)) {
    throw "FFI manifest not found: $ManifestPath"
}

$rustTarget = switch ($Arch) {
    "x64"   { "x86_64-pc-windows-msvc" }
    "arm64" { "aarch64-pc-windows-msvc" }
}

$cargoArgs = @("build")
if ($Release) {
    $cargoArgs += "--release"
}
$cargoArgs += @("--manifest-path", $ManifestPath, "--target", $rustTarget)

if (-not [string]::IsNullOrWhiteSpace($env:VSCMD_VER)) {
    & cargo @cargoArgs
    exit $LASTEXITCODE
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    throw "vswhere.exe not found: $vswhere"
}

$installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ([string]::IsNullOrWhiteSpace($installPath)) {
    throw "Visual Studio installation with C++ tools not found"
}

$vsDevCmd = Join-Path $installPath "Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path $vsDevCmd)) {
    throw "VsDevCmd.bat not found: $vsDevCmd"
}

$releasePart = if ($Release) { " --release" } else { "" }
$cargoCommand = "cargo build$releasePart --manifest-path `"$ManifestPath`" --target $rustTarget"
$cmd = "`"$vsDevCmd`" -arch=$Arch -host_arch=x64 && $cargoCommand"

cmd /d /c $cmd
exit $LASTEXITCODE
