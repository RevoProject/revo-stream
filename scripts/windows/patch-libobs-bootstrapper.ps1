$ErrorActionPreference = 'Stop'

$root = "$env:USERPROFILE\.cargo\git\checkouts"
$bootstrapper = Get-ChildItem $root -Recurse -Filter Cargo.toml |
  Where-Object { $_.FullName -like "*\7e76c91\libobs-bootstrapper\Cargo.toml" } |
  Select-Object -First 1
if (-not $bootstrapper) {
  throw "libobs-bootstrapper Cargo.toml not found"
}

$path = $bootstrapper.FullName
$dir = Split-Path $path
$cargo = Get-Content $path -Raw
Write-Host "Using bootstrapper: $dir"

if ($cargo -notmatch '(?ms)^\[features\]\s*(.*?)(?=^\[|\z)') {
  $cargo += "`r`n[features]`r`n"
}

if ($cargo -match '(?ms)^\[features\]\s*(.*?)(?=^\[|\z)') {
  if ($cargo -match '(?m)^default\s*=\s*\[([^\]]*)\]') {
    $defaults = $Matches[1]
    if ($defaults -notmatch '(^|,)\s*["'']install_dummy_dll["'']\s*(,|$)') {
      $newDefaults = $defaults.Trim()
      if ($newDefaults) {
        $newDefaults += ', '
      }
      $newDefaults += '"install_dummy_dll"'
      $cargo = [regex]::Replace(
        $cargo,
        '(?m)^default\s*=\s*\[[^\]]*\]',
        "default = [$newDefaults]",
        1
      )
    }
  }
  else {
    $cargo = [regex]::Replace(
      $cargo,
      '(?m)^\[features\]\s*',
      "[features]`r`ndefault = [""install_dummy_dll""]`r`n",
      1
    )
  }
}

if ($cargo -notmatch '(?m)^install_dummy_dll\s*=') {
  $cargo = [regex]::Replace(
    $cargo,
    '(?m)^\[features\]\s*',
    "[features]`r`ninstall_dummy_dll = []`r`n",
    1
  )
}

Set-Content -Path $path -Value $cargo -NoNewline

$verify = Get-Content $path -Raw
if ($verify -notmatch '(?m)^default\s*=\s*\[[^\]]*["'']install_dummy_dll["''][^\]]*\]') {
  throw "Failed to enable install_dummy_dll by default"
}

$dummy = Join-Path $dir "assets\obs-dummy.dll"
if (-not (Test-Path $dummy)) {
  throw "Dummy DLL not found: $dummy"
}

Write-Host "install_dummy_dll: enabled by default"
Write-Host "dummy DLL: $dummy"
