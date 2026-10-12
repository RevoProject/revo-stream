$ErrorActionPreference = 'Stop'

$target = "src-tauri/target/release"
$bundleDir = "$target/bundle"
$portable = "$bundleDir/revo-ui-windows-x64-portable"
$zip = "$bundleDir/revo-ui-windows-x64.zip"
$exe = "$target/revo-ui.exe"
$obsDll = "$target/obs.dll"
$root = "src-tauri/resources/revo-root"

Remove-Item $portable -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $zip -Force -ErrorAction SilentlyContinue
if (-not (Test-Path $exe)) { throw "Executable not found: $exe" }
if (-not (Test-Path $obsDll)) { throw "Real OBS DLL not found: $obsDll" }
if (-not (Test-Path $root)) { throw "revo-root not found: $root" }
$obsSize = (Get-Item $obsDll).Length
if ($obsSize -lt 500000) { throw "target/release/obs.dll is unexpectedly small; refusing to package dummy DLL" }

New-Item -ItemType Directory -Force $portable | Out-Null
Copy-Item $exe "$portable\revo-ui.exe" -Force
Copy-Item $obsDll "$portable\obs.dll" -Force

# Only root-level DLLs are duplicated here because Windows/bootstrapper
# needs them beside revo-ui.exe. Do not copy bin/data/obs-plugins again.
Get-ChildItem $target -Filter "*.dll" -File | ForEach-Object {
  Copy-Item $_.FullName "$portable\$($_.Name)" -Force
}
Copy-Item $root "$portable\revo-root" -Recurse -Force

if (-not (Test-Path "$portable\revo-root\core\lib\obs.dll")) { throw "Portable revo-root/core/lib/obs.dll missing" }
if (-not (Test-Path "$portable\revo-root\core\lib\obs-plugins")) { throw "Portable revo-root/core/lib/obs-plugins missing" }
$pluginCount = @(Get-ChildItem "$portable\revo-root\core\lib\obs-plugins" -Recurse -Filter "*.dll" -File).Count
if ($pluginCount -eq 0) { throw "No OBS plugins found in portable revo-root" }

Compress-Archive -Path "$portable\*" -DestinationPath $zip -CompressionLevel Optimal
if (-not (Test-Path $zip)) { throw "Portable ZIP was not created" }

$verifyDir = "$bundleDir\portable-verify"
Remove-Item $verifyDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $verifyDir | Out-Null
Expand-Archive -Path $zip -DestinationPath $verifyDir -Force
if (-not (Test-Path "$verifyDir\revo-ui.exe")) { throw "ZIP verification failed: revo-ui.exe missing" }
if (-not (Test-Path "$verifyDir\obs.dll")) { throw "ZIP verification failed: root obs.dll missing" }
if (-not (Test-Path "$verifyDir\revo-root\core\lib\obs.dll")) { throw "ZIP verification failed: revo-root/core/lib/obs.dll missing" }
$verifiedObsSize = (Get-Item "$verifyDir\obs.dll").Length
if ($verifiedObsSize -lt 500000) { throw "ZIP verification failed: root obs.dll is dummy/small" }
if (-not (Test-Path "$verifyDir\revo-root\core\lib\obs-plugins")) { throw "ZIP verification failed: OBS plugins missing" }
if (@(Get-ChildItem "$verifyDir\revo-root\core\lib\obs-plugins" -Recurse -Filter "*.dll" -File).Count -eq 0) { throw "ZIP verification failed: no OBS plugin DLLs" }
Write-Host "ZIP verification passed"
Remove-Item $verifyDir -Recurse -Force
Remove-Item $portable -Recurse -Force
