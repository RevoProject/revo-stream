# Windows gate for the framework-neutral core crates (revo-core, revo-gpui).
# Runs the same checks as scripts/core/verify.sh without the Tauri metadata
# step, which requires a Linux pkg-config environment.
$ErrorActionPreference = "Stop"

Set-Location (Join-Path $PSScriptRoot "..\..")

function Invoke-Step {
	param([string]$Name, [scriptblock]$Action)
	Write-Host ""
	Write-Host "==> $Name"
	& $Action
	if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Invoke-Step "git diff --check" { git diff --check }

Invoke-Step "cargo fmt --check (revo-core, revo-gpui)" {
	cargo fmt --manifest-path crates/revo-core/Cargo.toml -- --check
	cargo fmt --manifest-path crates/revo-gpui/Cargo.toml -- --check
}

Invoke-Step "cargo test (revo-core)" {
	cargo test --manifest-path crates/revo-core/Cargo.toml
}

Invoke-Step "cargo test (revo-gpui)" {
	cargo test --manifest-path crates/revo-gpui/Cargo.toml
}

Invoke-Step "cargo check (revo-gpui)" {
	cargo check --manifest-path crates/revo-gpui/Cargo.toml
}

Write-Host ""
Write-Host "==> all core checks passed"
