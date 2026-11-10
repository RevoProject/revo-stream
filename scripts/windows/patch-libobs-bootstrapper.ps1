$ErrorActionPreference = 'Stop'

$root = "$env:USERPROFILE\.cargo\git\checkouts"
# The workspace and the extracted backend can pin different libobs-rs
# revisions, so every fetched checkout is patched instead of a hard-coded one.
$bootstrappers = @(Get-ChildItem -LiteralPath $root -Recurse -Filter Cargo.toml -ErrorAction SilentlyContinue |
  Where-Object { $_.Directory.Name -eq 'libobs-bootstrapper' } |
  Where-Object {
    $revision = $_.Directory.Parent.Name
    $checkout = $_.Directory.Parent.Parent.Name
    ($revision -match '^[0-9a-f]{7,}$') -and ($checkout -like 'libobs-rs-*')
  })
if ($bootstrappers.Count -eq 0) {
  throw "libobs-bootstrapper Cargo.toml not found under $root"
}

foreach ($bootstrapper in $bootstrappers) {
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

  $dummy = Join-Path (Join-Path $dir 'assets') 'obs-dummy.dll'
  if (-not (Test-Path $dummy)) {
    throw "Dummy DLL not found: $dummy"
  }

  Write-Host "install_dummy_dll: enabled by default"
  Write-Host "dummy DLL: $dummy"
}
