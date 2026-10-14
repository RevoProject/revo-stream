$ErrorActionPreference = 'Stop'

$zip = "src-tauri/target/release/bundle/revo-ui-windows-x64.zip"
if (-not (Test-Path $zip)) { throw "Portable ZIP not found: $zip" }

$verifyDir = Join-Path $env:RUNNER_TEMP "revo-portable-smoke"
$proc = $null
try {
  Remove-Item $verifyDir -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $verifyDir | Out-Null
  Expand-Archive -Path $zip -DestinationPath $verifyDir -Force

  $exe = Join-Path $verifyDir "revo-ui.exe"
  if (-not (Test-Path $exe)) { throw "Portable executable is missing: $exe" }

  Write-Host "Launching portable application for 12 seconds..."
  $proc = Start-Process -FilePath $exe -PassThru -WorkingDirectory $verifyDir
  Start-Sleep -Seconds 12
  $proc.Refresh()
  if ($proc.HasExited) {
    throw "Portable application exited early with code $($proc.ExitCode)"
  }
  Write-Host "Portable smoke test passed"
}
finally {
  if ($proc -and -not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  }
  Remove-Item $verifyDir -Recurse -Force -ErrorAction SilentlyContinue
}
