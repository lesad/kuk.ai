# Contributing to kuk.ai

Thanks for your interest. kuk is small and opinionated — keep that in mind
before opening a large PR.

## Before you start

- **Bug fixes:** open a PR directly with a regression test.
- **New features or behavior changes:** open an issue first and propose the
  change. The CLI surface and exit codes are load-bearing — see `CLAUDE.md`
  for the invariants — so feature work needs alignment up front.
- **Skill changes** (`skills/kuk-compare/`): same — open an issue first.
  The skill has hard invariants (Figma desktop MCP required, `--scale 2`
  only, `sips -z` forbidden) that exist for documented reasons.

## Development

```sh
mise install           # one-time, pins Rust 1.96.0 via .mise.toml
cargo build --release
cargo test             # unit + integration (tests/cli.rs)
cargo clippy -- -D warnings
cargo fmt
```

Run `cargo clippy -- -D warnings` and `cargo fmt --check` before pushing.

## Commit messages

Conventional Commits format: `feat(scope): summary`, `fix: summary`,
`chore: summary`, `docs: summary`, etc. Look at `git log` for examples.

## Licensing

kuk is MPL-2.0. By submitting a PR you agree your contribution is licensed
under MPL-2.0. The license is file-level copyleft — if you modify an
existing MPL-licensed file, your changes stay MPL. You can still combine
kuk with proprietary code elsewhere in your stack.

We appreciate when downstream forks upstream their fixes, but the license
does not require PRs — only that modified MPL files remain MPL and remain
available.

## Code of conduct

Be decent. Disagreements are fine; personal attacks are not. Maintainer
reserves the right to close issues and PRs that are off-topic, abusive, or
not aligned with the project's scope.
