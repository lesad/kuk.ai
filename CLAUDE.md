# kuk.ai

Rust CLI (`kuk`) that compares two webpage screenshots — Figma design vs browser
implementation — and emits a similarity score + red-overlay diff PNG.
Bundles `skills/kuk-compare/` as the Claude Code skill that drives the full
Figma-MCP → fetch → capture → compare workflow.

## Commands

```bash
cargo build --release       # produces ./target/release/kuk
cargo test                  # unit (#[cfg(test)] per module) + integration (tests/cli.rs)
cargo run -- <design> <impl> --format toon
cargo clippy -- -D warnings
cargo fmt
```

Toolchain pinned via `.mise.toml` (Rust 1.96.0). Run `mise install` in a fresh
clone.

## Architecture

Single binary, package-by-concern modules under `src/`:

- `main.rs`    — entrypoint, exit-code mapping, mismatch printer
- `cli.rs`     — clap `Args`, `OutputFormat { Human, Json, Toon }`
- `compare.rs` — image-compare hybrid (MSSIM luma + RMS chroma + alpha)
- `overlay.rs` — per-pixel red-overlay renderer (uses `--gain`)
- `report.rs`  — three serializers: human, JSON, TOON (via `toon-format` crate + serde shim)
- `error.rs`   — `PeepError` (thiserror); `DimMismatch` is a distinct variant

`tests/cli.rs` — black-box integration via `assert_cmd` + `tempfile`, asserts on
stdout/exit codes. Add a new flag here when changing CLI surface.

## Exit codes (load-bearing — callers branch on these)

- `0` ok
- `1` threshold breach (only with `--fail`)
- `2` generic error
- `3` **dimension mismatch** — distinct on purpose, recoverable

Dimension-mismatch report goes to **stdout** in the selected format, not stderr.

## Skill — `skills/kuk-compare/`

Bundled Claude Code skill. Symlink into `~/.claude/skills/` to enable globally.
Driven by `SKILL.md`; helper script `scripts/figma-fetch.sh` (needs `curl`, `jq`,
`FIGMA_TOKEN` with `file_content:read` scope).

Invariants — do not casually relax:

- **Figma desktop MCP is required** in Step 1 (two gates: tools loaded + correct
  `fileKey` resolves). No REST-only fallback.
- **MCP image content blocks are vision-only.** Never try to forward MCP image
  bytes to `kuk` — always REST-fetch via `figma-fetch.sh`.
- **`--scale 2` is the only scale.** Matches Figma's default 2× export and
  `agent-browser` DPR=2. Don't add `--scale 1` examples.
- **`sips -z` is forbidden** (resize distorts pixel-accurate comparison).
  `sips -c` (crop) is last-resort only.
- Step 2 default impl-capture path is the `agent-browser` skill; Chrome DevTools
  is the manual fallback for auth-gated pages.

## Environment

- `FIGMA_TOKEN` — required by skill + `figma-fetch.sh`. Fish: `set -Ux FIGMA_TOKEN figd_...`
- `.mcp.json` wires the `figma-desktop` HTTP MCP at `http://127.0.0.1:3845/mcp`
  (Figma desktop app must be running).

## Release flow

1. Bump `version` in `Cargo.toml` (SemVer).
2. Rename `[Unreleased]` → `[x.y.z] - YYYY-MM-DD` in `CHANGELOG.md`.
3. Update the link footer at the bottom of `CHANGELOG.md`.
4. Commit `chore(release): x.y.z`, tag `vX.Y.Z`.

## Changelog

Keep `CHANGELOG.md` up to date on every meaningful change:

- Add entries under `[Unreleased]` as work progresses
- Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format:
  `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`
- On release: see "Release flow" above
