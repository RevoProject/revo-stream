# Contributing to RevoStream

Thanks for helping improve RevoStream. The project is in an active migration
from Tauri/Svelte to native GPUI, so read the current state before starting.

## Before you start

- Open an issue for bug reports and feature requests.
- Discuss major changes (architecture, dependencies, UI behavior, packaging)
  before implementing them.
- Keep pull requests focused on one logical change. Avoid drive-by renames,
  formatting churn, or unrelated cleanups.
- Start with `AGENTS.md` (current architecture and commands),
  `MIGRATIONS.md` (migration plan and parity contract), and
  [docs/building.md](docs/building.md) / [docs/scripts.md](docs/scripts.md).

## Licensing and open-source policy

RevoStream is licensed under **AGPL-3.0-only** (see [LICENSE](LICENSE)).

- **We do not accept closed-source code.** Contributions must be publishable
  under AGPL-3.0-only, and the resulting application must remain buildable from
  publicly available source.
- Do not submit proprietary code, leaked code, obfuscated code, or code you do
  not have the right to license under AGPL-3.0-only.
- Do not add proprietary or closed-source dependencies without prior approval
  from the maintainers.
- The project is and will remain open source. Some selected components may in
  the future be offered under a separate, possibly closed-source edition (for
  example optional integrations or hosted services). Code already released
  under AGPL-3.0-only stays available under that license.

By submitting a contribution, you confirm that you have the right to submit it
under AGPL-3.0-only and that it does not violate another project's license.

## AI-assisted contributions

AI tools are allowed, but they do not lower the review bar.

- **You are responsible for the result.** Understand the change, review it, and
  be able to explain why it is correct.
- **Tests must pass.** AI-assisted code is accepted only after confirmed tests
  and a green validation run. At minimum, run `bash scripts/core/verify.sh`; run
  the relevant platform build and smoke tests when the change touches libobs,
  packaging, or platform code.
- **No performance regressions.** AI-assisted changes must not degrade code
  quality or application performance. For performance-sensitive paths such as
  preview capture, frame delivery, audio, IPC, scene rendering, and startup,
  include before/after measurements or a benchmark.
- **Disclose substantial AI assistance** in the pull request description and
  list what was generated versus manually reviewed.
- **No unverifiable output.** Do not submit generated code whose licensing,
  provenance, or security properties you cannot establish. Never paste secrets,
  credentials, or private code into an AI tool and commit the result.
- **Keep it reviewable.** Split large generated changes into small, explainable
  commits instead of one bulk rewrite.

## Development workflow

The native GPUI application is the primary target. The legacy Tauri/Svelte path
is kept buildable as a parity reference until the final cutover.

- `bash scripts/core/verify.sh` — standard gate for `revo-core` and `revo-gpui`.
- `cargo test --manifest-path crates/revo-core/Cargo.toml`
- `cargo test --manifest-path crates/revo-gpui/Cargo.toml`
- `cargo check --manifest-path crates/revo-gpui/Cargo.toml`
- `cargo run --manifest-path crates/revo-gpui/Cargo.toml` — GPUI shell on mock
  data.
- `cargo build --manifest-path crates/revo-backend/Cargo.toml --bin revo-engine`
  plus `--features obs-backend -- --obs` — real libobs backend.

Commit messages follow the `<type>(<scope>): <subject>` format described in the
[EU Git Commit Guidelines](https://ec.europa.eu/component-library/v1.15.0/eu/docs/conventions/git/).
Use the imperative present tense, omit the trailing period, and keep the header
concise. Match the existing history, for example:

```text
feat(gpui): add transition preview controls
fix(windows): resolve the bundled engine path
docs: update the GPUI build guide
build: pin revo-lib to a fixed revision
```

Keep the README migration checklist and `AGENTS.md` in sync when a migration
checkpoint lands or the application version changes.

## Pull request checklist

- The PR has a clear scope and links to the relevant issue.
- Tests were added or updated for behavior changes.
- The relevant validation commands were run, and the results are included.
- Performance-sensitive changes include before/after evidence.
- UI changes preserve the RevoClassic/RevoFuture visual parity contract.
- Documentation was updated when commands, dependencies, or behavior changed.
- Any substantial AI assistance is disclosed.
- The contribution can be licensed under AGPL-3.0-only and contains no
  closed-source or proprietary code.

## Reporting bugs and security issues

- Include the commit or version, platform, steps to reproduce, expected and
  actual behavior, and relevant logs.
- Do not include secrets or personal data in issues or logs.
- Report security vulnerabilities privately through the repository's security
  advisory feature rather than a public issue.
