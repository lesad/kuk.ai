# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Renamed project from `peep` / `peep-rs` to `kuk` / `kuk.ai` — name conflict with an existing vendor. Binary: `peep` → `kuk`. Crate: `peep-rs` → `kuk-ai`. Repo: `lesad/peep-rs` → `lesad/kuk.ai`. Skill: `peep-compare` → `kuk-compare`.
- `kuk-compare` skill Step 1: MCP is now **required** — removed the Branch B REST-only fallback. Two hard gates before any REST call: (1) `mcp__figma-desktop__*` tools must be loaded, (2) `mcp__figma-desktop__get_metadata` must resolve the target `fileKey` (catches the common case where MCP is connected but the wrong file is open). Added render-timeout recovery: try a logical child node, then ask for manual export; never retry the same node at different scales.
- `kuk-compare` skill Step 1 token validation: replaced `/v1/me` probe (returns 403 for `file_content:read` scope) with the correct file-node endpoint check.
- `kuk-compare` skill Step 2 Branch A: replaced prose description with copy-paste `agent-browser` command sequence — `set viewport W H 2`, `eval --stdin` animation kill, `screenshot "[selector]" /tmp/impl.png`. Element capture at DPR=2 matches Figma's default `--scale 2` export directly; no full-page + sips crop needed.
- `kuk-compare` skill Step 2.5: removed manual dimension pre-check — kuk exit code 3 handles mismatches. Added exit-code-3 recovery guidance: fix via viewport/CSS width override; `sips -z` explicitly forbidden; `sips -c` (crop) last resort only; height mismatch treated as a real finding.
- `kuk-compare` skill: `sips` demoted throughout — crop-only last resort, never resize.
- `kuk-compare` skill: `--scale 2` hardened as the only scale throughout — removed `--scale N` guidance and `--scale 1` example; always match Figma's default 2× export.
- `kuk-compare` skill Step 1: mandatory MCP visual confirmation before any REST call. When the Figma desktop MCP is available, the agent must screenshot the candidate node via `mcp__figma-desktop__get_screenshot`, show it to the user, and only call `figma-fetch.sh` after explicit confirmation. Wrong-node REST fetches now require user error, not agent guessing. When MCP is unavailable, Branch B falls back to a best-effort flow: prefer asking the user for a clean node-id URL, but if only a frame name is available, restate the interpretation and proceed with a single REST call — degraded validation is still better than no fetch.
- `kuk-compare` skill: new Step 2.5 pre-flight smoke test. Agent vision-inspects both PNGs before invoking kuk to catch wildly-wrong impl captures (wrong theme, wrong viewport, wrong page, loading state, locale mismatch). Reports a re-capture request to the user instead of running kuk when red flags are present — no point computing MSSIM on two obviously-different images.
- `kuk-compare` skill Step 2: `agent-browser` skill promoted to the default impl-capture path — programmable viewport, element-by-CSS-selector screenshot, and CSS injection for diagnostic loops (hide unimplemented elements, kill animations, pin draggables, bisect bugs with in-place style tweaks). Chrome DevTools manual flow stays as a fallback for auth-gated or otherwise unreachable pages. Removed the stale duplicate `peep` invocation that had crept into Step 2 — Step 2 ends at `/tmp/impl.png`, Step 3 still owns the compare.

## [0.2.0] - 2026-06-05

### Added

- `tools/figma-fetch.sh` — bash helper that fetches a Figma node as PNG (or JPG/SVG/PDF) via the REST `/v1/images` endpoint. Writes to a self-describing default path under `$TMPDIR` and prints the path on stdout for shell capture; supports `--scale`, `--format`, `--absolute/--no-absolute`, and `--out PATH|-`. Requires `FIGMA_TOKEN` with `file_content:read` scope plus `curl` and `jq`.
- `--format <human|json|toon>` flag on `peep` selecting output format. Default `human`.
- Both inputs' dimensions are echoed on every successful run (in whichever format was selected).
- TOON output via `--format toon` for token-efficient agent consumption. Uses the official `toon-format` crate behind a small serde shim that preserves the tabular `sources[2]{label,path,width,height}` shape.
- Exit code `3` for dimension mismatch, distinct from generic error code `2`. Recoverable — callers can branch on it.

### Changed

- `kuk-compare` skill: Step 1 rewritten to use hybrid Figma desktop MCP navigation plus REST fetch via `tools/figma-fetch.sh`. Documented that MCP image content blocks are vision-only and cannot be proxied to peep.
- `kuk-compare` skill: Step 2 rewritten to use Chrome (or any Chromium-family) DevTools "Capture node screenshot" as the sole impl capture mechanism. The latest `~/Downloads/*.png` is copied to `/tmp/impl.png`. No new bundled tools.
- `kuk-compare` skill: Step 3, CI Path, and Flags Reference table migrated off the removed `--json` flag to `--format json`/`toon`.
- Dimension-mismatch report now goes to **stdout** in the selected format (was free-text on stderr in `0.1.0`). Same stream as the success report so callers parse once.

### Removed

- `--json` flag on `peep`. Replaced by `--format json`. No alias.

## [0.1.0] - 2026-06-04

### Added

- CLI tool `peep` to compare two webpage screenshots and produce a similarity score
- Red-overlay diff image output highlighting pixel differences
- JSON output support via `--json` flag
- Configurable similarity threshold via `--threshold` flag
- Exit code reflects pass/fail based on threshold

[Unreleased]: https://github.com/lesad/kuk.ai/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/lesad/kuk.ai/releases/tag/v0.2.0
[0.1.0]: https://github.com/lesad/kuk.ai/releases/tag/v0.1.0
