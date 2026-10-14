$ErrorActionPreference = 'Stop'

$ROOT = "src-tauri/resources/revo-root"
$required = @(
  "$ROOT\core\lib\obs.dll",
  "$ROOT\core\bin\obs-ffmpeg-mux.exe",
  "$ROOT\core\lib\obs-plugins\obs-nvenc.dll",
  "$ROOT\core\lib\obs-plugins",
  "$ROOT\core\share\obs\libobs\default.effect",
  "$ROOT\data\share\obs\libobs\default.effect"
)
foreach ($path in $required) {
  if (-not (Test-Path $path)) {
    throw "Required runtime path missing: $path"
  }
  Write-Host "OK: $path"
}
$dllCount = @(
  Get-ChildItem `
    "$ROOT\core\lib" `
    -Filter "*.dll" `
    -File
).Count
$pluginCount = @(
  Get-ChildItem `
    "$ROOT\core\lib\obs-plugins" `
    -Recurse `
    -Filter "*.dll" `
    -File
).Count
Write-Host "Core DLLs: $dllCount"
Write-Host "Plugin DLLs: $pluginCount"
if ($dllCount -eq 0) {
  throw "No core DLLs staged"
}
if ($pluginCount -eq 0) {
  throw "No plugin DLLs staged"
}
if (Test-Path "$ROOT\core\lib\obs-plugins\64bit") {
  throw "Plugins must be flattened into core/lib/obs-plugins"
}
Write-Host "Windows runtime is complete"
