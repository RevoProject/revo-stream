[CmdletBinding()]
param(
    [string]$ObsOut = "src-tauri/target/release",
    [string]$Root = "src-tauri/resources/revo-root",
    [string]$RepoData = "data"
)

$ErrorActionPreference = "Stop"
$ObsOut = (Resolve-Path $ObsOut).Path
if (Test-Path $Root) { Remove-Item $Root -Recurse -Force }

foreach ($relative in @(
    "data", "data\share\obs", "core\bin", "core\lib",
    "core\lib\obs-plugins", "core\share\obs"
)) {
    New-Item -ItemType Directory -Force (Join-Path $Root $relative) | Out-Null
}

if (Test-Path $RepoData) {
    Copy-Item (Join-Path $RepoData "*") (Join-Path $Root "data") -Recurse -Force
}

$dlls = @(Get-ChildItem $ObsOut -Filter "*.dll" -File)
if ($dlls.Count -eq 0) { throw "No OBS runtime DLLs found in $ObsOut" }
foreach ($dll in $dlls) {
    Copy-Item $dll.FullName (Join-Path $Root "core\lib\$($dll.Name)") -Force
}

$exes = @(Get-ChildItem $ObsOut -Filter "*.exe" -File | Where-Object { $_.Name -notlike "revo-ui*" })
foreach ($exe in $exes) {
    Copy-Item $exe.FullName (Join-Path $Root "core\bin\$($exe.Name)") -Force
}
if (-not (Test-Path (Join-Path $Root "core\bin\obs-ffmpeg-mux.exe"))) {
    throw "obs-ffmpeg-mux.exe is missing from the OBS output"
}

$pluginSource = Join-Path $ObsOut "obs-plugins"
if (-not (Test-Path $pluginSource)) { throw "OBS plugin directory missing: $pluginSource" }
$pluginBin = Get-ChildItem $pluginSource -Directory |
    Where-Object { @(Get-ChildItem $_.FullName -Filter "*.dll" -File).Count -gt 0 } |
    Select-Object -First 1
if (-not $pluginBin) { $pluginBin = Get-Item $pluginSource }
$excluded = @("libEGL.dll", "libGLESv2.dll", "libEGL.dll.lib", "libGLESv2.dll.lib")
foreach ($plugin in Get-ChildItem $pluginBin.FullName -Filter "*.dll" -File) {
    if ($excluded -notcontains $plugin.Name) {
        Copy-Item $plugin.FullName (Join-Path $Root "core\lib\obs-plugins\$($plugin.Name)") -Force
    }
}

$obsData = Join-Path $ObsOut "data"
if (-not (Test-Path $obsData)) { throw "OBS data directory missing: $obsData" }
Copy-Item (Join-Path $obsData "*") (Join-Path $Root "core\share\obs") -Recurse -Force
Copy-Item (Join-Path $obsData "*") (Join-Path $Root "data\share\obs") -Recurse -Force

$required = @(
    (Join-Path $Root "core\lib\obs.dll"),
    (Join-Path $Root "core\bin\obs-ffmpeg-mux.exe"),
    (Join-Path $Root "core\lib\obs-plugins\obs-nvenc.dll"),
    (Join-Path $Root "core\share\obs\libobs\default.effect"),
    (Join-Path $Root "data\share\obs\libobs\default.effect")
)
foreach ($path in $required) {
    if (-not (Test-Path $path)) { throw "Required Windows runtime path missing: $path" }
}
$pluginCount = @(Get-ChildItem (Join-Path $Root "core\lib\obs-plugins") -Filter "*.dll" -File).Count
if ($pluginCount -eq 0) { throw "No flat OBS plugin DLLs were staged" }
Write-Host "Staged Windows OBS runtime: $pluginCount plugins"
