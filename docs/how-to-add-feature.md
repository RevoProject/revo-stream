# How to add a feature (practical guide)

## General rule
Small, focused PR. One logical change. Conventional Commits.

## Path A: new backend command
1. Add a variant to `BackendCommand` in `crates/revo-core/src/service.rs`.
2. Add handling in the engine (`crates/revo-backend`).
3. Update `BackendSnapshot` if new state is needed.
4. Add a test in `revo-core` (if the logic is pure).
5. Call the command from the UI (GPUI action).
6. Run `bash scripts/core/verify.sh`.

## Path B: new view/modal in GPUI
1. Add a component in `crates/revo-gpui/src/components/...`.
2. Register it in `mod.rs` and the modal routing.
3. Use the shared theme (RevoClassic/RevoFuture).
4. Preserve layout/spacing (parity contract).
5. Add view tests where possible.

## Path C: new setting
1. Add a field to `AppSettings` (`crates/revo-core/src/models.rs`).
2. Set a default in `crates/revo-core/src/settings.rs`.
3. Handle it in the backend (load/save).
4. Add UI in the settings modal.
5. Update the snapshot if needed.

## Checklist before PR
- [ ] `bash scripts/core/verify.sh` passes
- [ ] Tests added/updated
- [ ] Docs updated (if behavior changes)
- [ ] UI: RevoClassic/RevoFuture at 1080x600, 1280x720, 1440x900
- [ ] Performance-sensitive: before/after (if applicable)
- [ ] No secrets in logs/screenshots

## Where to ask
- Issues: feature request template
- PR: template with checklist
- Docs: `docs/onboarding.md`, `docs/architecture.md`