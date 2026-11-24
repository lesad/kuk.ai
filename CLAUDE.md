# kuk.ai

Rust CLI (`kuk`) that compares two webpage screenshots, a Figma design and a
browser implementation, and emits a similarity score plus a red-overlay diff
PNG. It bundles `skills/kuk-compare/`, the Claude Code skill that drives the
full Figma MCP, fetch, capture and compare workflow.

Current version: 0.3.1 (early beta). The CLI core path is stable; the skill is
still changing. Breaking changes are allowed until 1.0, but they go in the
changelog.

## Commands

```bash
mise install                              # fresh clone: installs the pinned Rust
cargo build --release                     # produces ./target/release/kuk
cargo test                                # unit (per module) + integration (tests/cli.rs)
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -- <design> <impl> --format toon
```

The toolchain is pinned in `.mise.toml` (Rust 1.96.0). CI
(`.github/workflows/ci.yml`) runs the same fmt and clippy commands, then
`cargo build --release --locked` and `cargo test --release --locked` on Ubuntu
and macOS. Run all four checks before calling work done.

## Architecture

One binary, one module per concern under `src/`:

- `main.rs`: entrypoint and exit-code mapping
- `cli.rs`: clap `Args`, `OutputFormat { Text, Json, Toon }`, the `completions` and `schema` subcommands, the exit-code table
- `compare.rs`: `image-compare` hybrid (MSSIM on luma, RMS on chroma, alpha)
- `overlay.rs`: per-pixel red-overlay renderer (uses `--gain`)
- `report.rs`: `Report` and `Mismatch`, each rendered as text, JSON or TOON (`toon-format` crate through a serde shim), and `SCHEMA_VERSION`
- `schema.rs`: `kuk schema`, built from the clap definitions and real payloads
- `error.rs`: `KukError` (thiserror); `DimMismatch` is a distinct variant

`tests/cli.rs` is black-box integration through `assert_cmd` and `tempfile`. It
asserts on stdout and exit codes. Any change to the CLI surface adds a test
there.

## Exit codes

Callers branch on these, so they are a contract:

- `0` ok
- `1` threshold breach (only with `--fail`)
- `2` generic error
- `3` dimension mismatch, distinct on purpose because it is recoverable

The dimension-mismatch report goes to stdout in the selected format, not to
stderr.

## CLI design rules

The shared CLI design rules apply here. How `kuk` meets them:

- `kuk <design> <impl>` stays the main form. `completions <shell>` and `schema`
  are subcommands next to it. New capabilities are subcommands, not more flags
  on the compare form.
- `--format text|json|toon`, with `--json` as the shorthand. `human` is a
  hidden alias for `text`; do not remove it before 1.0.
- JSON and TOON payloads carry `schema_version` (`report::SCHEMA_VERSION`).
  Add fields freely. Removing or renaming one bumps it and is a major version.
- `kuk schema` and the `--help` footer read the exit codes from
  `cli::EXIT_CODES`. Change the table, not the prose.
- Paths with control characters are refused at parse time. Path traversal is
  not checked: `kuk` reads and writes the paths its caller names, by design.
- An error prints what failed, the cause chain, and a `hint:` line when there
  is a next step (`KukError::hint`).

Known gaps: no man page (`clap_mangen`), and text is the default even when
stdout is not a terminal, so agents must pass `--format toon`.

## Skill: `skills/kuk-compare/`

Driven by `SKILL.md`, with the helper `scripts/figma-fetch.sh` (needs `curl`,
`jq` and `FIGMA_TOKEN` with the `file_content:read` scope). To enable it
globally, symlink it:

```bash
ln -s "$PWD/skills/kuk-compare" ~/.claude/skills/kuk-compare
```

Invariants. Do not relax them without a reason that goes in the changelog:

- Figma desktop MCP is required in Step 1. Two gates: the tools are loaded, and
  the correct `fileKey` resolves. There is no REST-only fallback.
- MCP image content blocks are vision-only. Never try to forward MCP image
  bytes to `kuk`; always fetch over REST with `figma-fetch.sh`.
- `--scale 2` is the only scale. It matches Figma's default 2x export and
  `agent-browser` at DPR 2. Do not add `--scale 1` examples.
- `sips -z` is forbidden, because a resize distorts a pixel-accurate
  comparison. `sips -c` (crop) is a last resort.
- The default implementation capture in Step 2 is the `agent-browser` skill.
  Chrome DevTools is the manual fallback for pages it cannot reach.

The skill's `agent-browser` commands (`set viewport W H 2`, `eval --stdin`,
`screenshot <selector> <path>`) were last checked against `agent-browser`
0.33.1. Re-check them with `agent-browser <command> --help` when the skill
changes.

## Environment

- `FIGMA_TOKEN`: required by the skill and `figma-fetch.sh`. Fish:
  `set -Ux FIGMA_TOKEN figd_...`. Never print it.
- `.mcp.json` wires the `figma-desktop` HTTP MCP at `http://127.0.0.1:3845/mcp`.
  The Figma desktop app must be running, or the server fails to connect.
- `/.design-browser/` is a persistent `agent-browser` profile
  (`--profile ./.design-browser`). It holds session cookies and is gitignored.
  Never commit or read it.

## Current direction: point queries

Experiment, not designed yet. Today the agent reads the red overlay with vision
and guesses which element is wrong. The goal is to let it ask about one red
point and get the stack of elements and components under it, straight from the
browser.

Starting facts, to be confirmed by the experiment:

- The diff PNG is in device pixels at DPR 2. A diff pixel `(px, py)` in an
  element capture maps to CSS coordinates `(box.x + px / 2, box.y + py / 2)`,
  where `box` comes from `agent-browser get box <selector>`.
- `document.elementsFromPoint(x, y)` through `agent-browser eval` returns the
  DOM stack at viewport coordinates, so the target must be scrolled into view.
- `agent-browser open --enable react-devtools` adds `react tree` and
  `react inspect <id>` for the component side.
- `kuk` reports one score today and no regions. Clustering red pixels into
  regions with coordinates would give the agent points to ask about.
- Element and component stacks are large, uniform and nested when they come
  out of the browser as JSON. Project them to flat rows (depth, tag, test id,
  component, box) and emit TOON by default, per the token budget rules. Never
  hand the agent raw `react inspect` or DOM JSON.

Open question: what belongs in `kuk` (regions, coordinates) and what belongs
in the skill (browser queries). Settle it in a spec before writing code.

## Release flow

1. Bump `version` in `Cargo.toml` (SemVer).
2. Rename `[Unreleased]` to `[x.y.z] - YYYY-MM-DD` in `CHANGELOG.md`.
3. Update the link footer at the bottom of `CHANGELOG.md`.
4. Commit `chore(release): x.y.z` and tag `vX.Y.Z`.

## Changelog

Keep `CHANGELOG.md` current on every meaningful change:

- Add entries under `[Unreleased]` as the work progresses.
- Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/):
  `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`.
- On release, follow the release flow above.
