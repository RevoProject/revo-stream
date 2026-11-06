#!/usr/bin/env bash
# Validation pipeline for the framework-neutral core crates (revo-core,
# revo-gpui). Runs the narrowest checks that do not require GTK/WebKit/libobs
# host packages. Intended as the standard gate for every core migration slice.
set -euo pipefail

cd "$(dirname "$0")/../.."

step() {
	printf '\n==> %s\n' "$1"
}

step "git diff --check"
git diff --check

step "cargo fmt --check (revo-core, revo-gpui)"
cargo fmt --manifest-path crates/revo-core/Cargo.toml -- --check
cargo fmt --manifest-path crates/revo-gpui/Cargo.toml -- --check

step "cargo test (revo-core)"
cargo test --manifest-path crates/revo-core/Cargo.toml

step "cargo test (revo-gpui)"
cargo test --manifest-path crates/revo-gpui/Cargo.toml

step "cargo check (revo-gpui)"
cargo check --manifest-path crates/revo-gpui/Cargo.toml

step "cargo metadata (src-tauri compatibility seam)"
cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1 >/dev/null

printf '\n==> all core checks passed\n'
