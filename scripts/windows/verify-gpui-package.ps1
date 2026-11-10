# Verifies the GPUI Windows artifacts: every package exists, the portable ZIP
# extracts to a complete tree, and both binaries are x64 PE images.
param(
    [string]$Root = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)))
)

$ErrorActionPreference = "Stop"
Set-Location $Root

$BundleDir = "crates\revo-gpui\target\release\bundle"
$Version = (Get-Content "crates\revo-gpui\Cargo.toml" | Select-String -Pattern '^version = "([^"]+)"' |
    Select-Object -First 1).Matches.Groups[1].Value
if (-not $Version) { throw "could not read the version" }
$Prefix = "revo-gpui-$Version-windows-x64"

function MachineOfPe([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $reader = New-Object System.IO.BinaryReader($stream)
        $span = $reader.ReadBytes(2)
        if (($span[0] -ne 0x4D) -or ($span[1] -ne 0x5A)) { throw "not a PE file: $Path" }
        $stream.Position = 0x3C
        $headerOffset = $reader.ReadInt32()
        $stream.Position = $headerOffset + 4
        return $reader.ReadUInt16()
    } finally { $stream.Dispose() }
}

Write-Host "=== Checking artifacts ==="
foreach ($name in @("$Prefix.zip", "$Prefix.exe", "$Prefix.msi")) {
    $path = Join-Path $BundleDir $name
    if (-not (Test-Path $path)) { throw "missing artifact: $name" }
    $size = (Get-Item $path).Length
    if ($size -lt 1MB) { throw "artifact looks truncated: $name ($size bytes)" }
    Write-Host ("  {0,-46} {1,10:N0} bytes" -f $name, $size)
}

Write-Host "=== Checking portable ZIP contents ==="
$extract = Join-Path $env:RUNNER_TEMP "gpui-zip-check"
if (Test-Path $extract) { Remove-Item -Recurse -Force $extract }
Expand-Archive -Path (Join-Path $BundleDir "$Prefix.zip") -DestinationPath $extract
$root = Join-Path $extract "revo-gpui-$Version-windows-x64"
foreach ($required in @("revo-gpui.exe", "revo-engine.exe", "revo-root\core\lib\obs-plugins", "README.txt")) {
    if (-not (Test-Path (Join-Path $root $required))) { throw "portable tree is missing $required" }
    Write-Host "  ok: $required"
}

Write-Host "=== Checking binary architecture ==="
foreach ($binary in @("revo-gpui.exe", "revo-engine.exe")) {
    $machine = MachineOfPe (Join-Path $root $binary)
    if ($machine -ne 0x8664) { throw "$binary is not an x64 image (machine 0x{0:X4})" -f $machine }
    Write-Host ("  ok: {0} (x64 PE, machine 0x{1:X4})" -f $binary, $machine)
}

Write-Host "GPUI Windows artifacts are complete"
