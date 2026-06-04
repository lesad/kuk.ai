# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `skills/peep-compare/` — bundled Claude Code skill for visual comparison workflow. Contains `SKILL.md` and `scripts/figma-fetch.sh`, a bash helper that fetches a Figma node as PNG (or JPG/SVG/PDF) via the REST `/v1/images` endpoint. Writes to a self-describing default path under `$TMPDIR` and prints the path on stdout for shell capture; supports `--scale`, `--format`, `--absolute/--no-absolute`, and `--out PATH|-`. Requires `FIGMA_TOKEN` with `file_content:read` scope plus `curl` and `jq`. To enable globally: `ln -s "$PWD/skills/peep-compare" ~/.claude/skills/peep-compare`.

## [0.1.0] - 2026-06-04

### Added

- CLI tool `peep` to compare two webpage screenshots and produce a similarity score
- Red-overlay diff image output highlighting pixel differences
- JSON output support via `--json` flag
- Configurable similarity threshold via `--threshold` flag
- Exit code reflects pass/fail based on threshold

[Unreleased]: https://github.com/lesad/peep-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lesad/peep-rs/releases/tag/v0.1.0
