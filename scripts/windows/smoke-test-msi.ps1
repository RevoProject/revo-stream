$ErrorActionPreference = 'Stop'

$msi = Get-ChildItem "src-tauri/target/release/bundle/msi" -Filter "*.msi" -File |
  Select-Object -First 1
if (-not $msi) { throw "MSI was not produced" }

$installDir = Join-Path ${env:ProgramFiles} "revo-ui"
$installedExe = Join-Path $installDir "revo-ui.exe"
$installLog = Join-Path $env:RUNNER_TEMP "revo-msi-install.log"
$uninstallLog = Join-Path $env:RUNNER_TEMP "revo-msi-uninstall.log"
$proc = $null
$installed = $false

function Invoke-Msi {
  param([string[]]$Arguments)
  $result = Start-Process -FilePath "msiexec.exe" `
    -ArgumentList $Arguments -Wait -PassThru -WindowStyle Hidden
  if ($result.ExitCode -ne 0 -and $result.ExitCode -ne 3010) {
    throw "msiexec failed with exit code $($result.ExitCode)"
  }
}

try {
  Write-Host "Installing MSI: $($msi.FullName)"
  Invoke-Msi @("/i", "`"$($msi.FullName)`"", "/qn", "/norestart", "/L*v", "`"$installLog`"")
  $installed = $true

  if (-not (Test-Path $installedExe)) {
    throw "Installed executable is missing: $installedExe"
  }

  Write-Host "Launching installed application for 12 seconds..."
  $proc = Start-Process -FilePath $installedExe -PassThru -WorkingDirectory $installDir
  Start-Sleep -Seconds 12
  $proc.Refresh()
  if ($proc.HasExited) {
    throw "Installed application exited early with code $($proc.ExitCode)"
  }
  Write-Host "MSI application stayed alive"
}
finally {
  if ($proc -and -not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  }
  if ($installed) {
    Write-Host "Uninstalling MSI..."
    Invoke-Msi @("/x", "`"$($msi.FullName)`"", "/qn", "/norestart", "/L*v", "`"$uninstallLog`"")
  }
}

if (Test-Path $installedExe) {
  throw "Installed executable remains after uninstall: $installedExe"
}
Write-Host "MSI smoke test passed"
