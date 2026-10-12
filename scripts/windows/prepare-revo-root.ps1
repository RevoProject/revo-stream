$ErrorActionPreference = 'Stop'

Remove-Item `
  "src-tauri/resources/revo-root" `
  -Recurse `
  -Force `
  -ErrorAction SilentlyContinue
New-Item `
  -ItemType Directory `
  -Force `
  "src-tauri/resources/revo-root" | Out-Null
