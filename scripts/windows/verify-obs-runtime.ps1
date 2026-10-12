$ErrorActionPreference = 'Stop'

$out = "src-tauri/target/release"
$obs = "$out/obs.dll"
if (-not (Test-Path $obs)) {
  throw "OBS runtime missing: $obs"
}
$size = (Get-Item $obs).Length
Write-Host "obs.dll size: $size bytes"
if ($size -lt 500000) {
  throw "obs.dll is unexpectedly small"
}
$plugins = "$out/obs-plugins"
if (-not (Test-Path $plugins)) {
  throw "OBS plugin directory missing: $plugins"
}
$pluginCount = @(
  Get-ChildItem `
    $plugins `
    -Recurse `
    -Filter "*.dll" `
    -File
).Count
Write-Host "OBS plugin DLLs: $pluginCount"
if ($pluginCount -eq 0) {
  throw "No OBS plugin DLLs found"
}
if (Test-Path "$out/data") {
  Write-Host "OBS data directory found"
}
else {
  throw "OBS data directory missing"
}
Write-Host "Windows OBS runtime looks valid"
