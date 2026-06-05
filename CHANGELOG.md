# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-05

### Added

- `tools/figma-fetch.sh` — bash helper that fetches a Figma node as PNG (or JPG/SVG/PDF) via the REST `/v1/images` endpoint. Writes to a self-describing default path under `$TMPDIR` and prints the path on stdout for shell capture; supports `--scale`, `--format`, `--absolute/--no-absolute`, and `--out PATH|-`. Requires `FIGMA_TOKEN` with `file_content:read` scope plus `curl` and `jq`.
- `--format <human|json|toon>` flag on `peep` selecting output format. Default `human`.
- Both inputs' dimensions are echoed on every successful run (in whichever format was selected).
- TOON output via `--format toon` for token-efficient agent consumption. Uses the official `toon-format` crate behind a small serde shim that preserves the tabular `sources[2]{label,path,width,height}` shape.
- Exit code `3` for dimension mismatch, distinct from generic error code `2`. Recoverable — callers can branch on it.

### Changed

- `peep-compare` skill: Step 1 rewritten to use hybrid Figma desktop MCP navigation plus REST fetch via `tools/figma-fetch.sh`. Documented that MCP image content blocks are vision-only and cannot be proxied to peep.
- `peep-compare` skill: Step 2 rewritten to use Chrome (or any Chromium-family) DevTools "Capture node screenshot" as the sole impl capture mechanism. The latest `~/Downloads/*.png` is copied to `/tmp/impl.png`. No new bundled tools.
- `peep-compare` skill: Step 3, CI Path, and Flags Reference table migrated off the removed `--json` flag to `--format json`/`toon`.
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

[Unreleased]: https://github.com/lesad/peep-rs/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/lesad/peep-rs/releases/tag/v0.2.0
[0.1.0]: https://github.com/lesad/peep-rs/releases/tag/v0.1.0
