$ErrorActionPreference = 'Stop'

$msi = Get-ChildItem `
  "src-tauri/target/release/bundle/msi" `
  -Filter "*.msi" `
  -File `
  -ErrorAction SilentlyContinue
if (-not $msi) {
  throw "MSI was not produced"
}
$msi | ForEach-Object {
 Write-Host "MSI: $($_.FullName)"
 }
