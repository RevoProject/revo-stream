# Enable the libobs-bootstrapper dummy-DLL feature for the revision this build
# actually compiles.
#
# `revo-lib` tracks libobs-rs `branch = "main"` with no rev, so the compiled
# revision comes from the lockfiles. Older revisions ship assets/obs-dummy.dll
# and need `install_dummy_dll` on by default so the bootstrap does not have to
# download OBS during a build; newer revisions dropped the dummy DLL entirely,
# and for those there is nothing to enable.
$ErrorActionPreference = 'Stop'

$root = "$env:USERPROFILE\.cargo\git\checkouts"
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

$pattern = 'git\+https://github\.com/libobs-rs/libobs-rs\?branch=main#([0-9a-f]+)'
$revisions = @()
foreach ($lock in @('src-tauri\Cargo.lock', 'crates\revo-backend\Cargo.lock', 'crates\revo-gpui\Cargo.lock')) {
  $path = Join-Path $projectRoot $lock
  if (-not (Test-Path $path)) { continue }
  foreach ($match in [regex]::Matches((Get-Content $path -Raw), $pattern)) {
    $revisions += $match.Groups[1].Value.Substring(0, 7)
  }
}
$revisions = @($revisions | Sort-Object -Unique)
if ($revisions.Count -eq 0) {
  throw 'no libobs-rs revision is pinned by the committed lockfiles'
}
Write-Host "pinned libobs-rs revisions: $($revisions -join ', ')"

$patched = 0
$skipped = 0
foreach ($revision in $revisions) {
  $manifest = Get-ChildItem -LiteralPath $root -Recurse -Filter Cargo.toml -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -eq 'libobs-bootstrapper' } |
    Where-Object { $_.Directory.Parent.Name -eq $revision } |
    Where-Object { $_.Directory.Parent.Parent.Name -like 'libobs-rs-*' } |
    Select-Object -First 1
  if (-not $manifest) {
    Write-Warning "revision $revision is not fetched in this job"
    continue
  }

  $dir = $manifest.Directory.FullName
  $dummy = Join-Path (Join-Path $dir 'assets') 'obs-dummy.dll'
  if (-not (Test-Path $dummy)) {
    Write-Host "$dir ships no dummy DLL: nothing to enable, skipped"
    $skipped++
    continue
  }

  $path = $manifest.FullName
  Write-Host "Using bootstrapper: $dir"
  $cargo = Get-Content $path -Raw

  if ($cargo -notmatch '(?ms)^\[features\]\s*(.*?)(?=^\[|\z)') {
    $cargo += "`r`n[features]`r`n"
  }

  if ($cargo -match '(?m)^default\s*=\s*\[([^\]]*)\]') {
    $defaults = $Matches[1]
    if ($defaults -notmatch '(^|,)\s*["'']install_dummy_dll["'']\s*(,|$)') {
      $newDefaults = $defaults.Trim()
      if ($newDefaults) { $newDefaults += ', ' }
      $newDefaults += '"install_dummy_dll"'
      $cargo = [regex]::Replace($cargo, '(?m)^default\s*=\s*\[[^\]]*\]', "default = [$newDefaults]", 1)
    }
  }
  else {
    $cargo = [regex]::Replace($cargo, '(?m)^\[features\]\s*', "[features]`r`ndefault = [""install_dummy_dll""]`r`n", 1)
  }

  if ($cargo -notmatch '(?m)^install_dummy_dll\s*=') {
    $cargo = [regex]::Replace($cargo, '(?m)^\[features\]\s*', "[features]`r`ninstall_dummy_dll = []`r`n", 1)
  }

  Set-Content -Path $path -Value $cargo -NoNewline

  $verify = Get-Content $path -Raw
  if ($verify -notmatch '(?m)^default\s*=\s*\[[^\]]*["'']install_dummy_dll["''][^\]]*\]') {
    throw "Failed to enable install_dummy_dll by default in $path"
  }
  Write-Host "install_dummy_dll: enabled by default"
  Write-Host "dummy DLL: $dummy"
  $patched++
}

if ($patched -eq 0 -and $skipped -eq 0) {
  throw "none of the pinned libobs-rs revisions are fetched under $root"
}
Write-Host "bootstrapper patched in $patched revision(s), skipped $skipped"
