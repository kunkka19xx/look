# Runs while Look is still open: any failure is reported back to the app and
# Look keeps running.
param(
    [Parameter(Mandatory)] [string] $SetupUrl,
    [Parameter(Mandatory)] [string] $ChecksumsUrl,
    [Parameter(Mandatory)] [string] $SetupPath
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$SetupTimeoutSec = 300
$ChecksumsTimeoutSec = 60
$SetupFileName = Split-Path -Leaf $SetupPath
$ChecksumsPath = "$SetupPath.checksums.txt"

$failure = $null
try {
    Invoke-WebRequest -Uri $SetupUrl -OutFile $SetupPath -UseBasicParsing -TimeoutSec $SetupTimeoutSec
    Invoke-WebRequest -Uri $ChecksumsUrl -OutFile $ChecksumsPath -UseBasicParsing -TimeoutSec $ChecksumsTimeoutSec
    $expected = $null
    foreach ($line in Get-Content $ChecksumsPath) {
        $parts = $line.Trim() -split '\s+', 2
        if ($parts.Count -eq 2 -and $parts[1].Trim().TrimStart('*') -eq $SetupFileName) {
            $expected = $parts[0].ToLower()
            break
        }
    }
    if (-not $expected) { throw "Checksums file has no entry for '$SetupFileName'." }
    $actual = (Get-FileHash -Path $SetupPath -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) { throw "SHA256 mismatch. expected=$expected actual=$actual" }
} catch {
    $failure = $_.Exception.Message
}
Remove-Item -Path $ChecksumsPath -Force -ErrorAction SilentlyContinue
if ($failure) {
    Remove-Item -Path $SetupPath -Force -ErrorAction SilentlyContinue
    [Console]::Error.WriteLine($failure)
    exit 1
}
