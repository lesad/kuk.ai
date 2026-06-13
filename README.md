```
▄▄           ▄▄              ▀▀
██ ▄█▀ ██ ██ ██ ▄█▀     ▀▀█▄ ██
████   ██ ██ ████      ▄█▀██ ██
██ ▀█▄ ▀██▀█ ██ ▀█▄ ██ ▀█▄██ ██▄
```

# kuk.ai

> [!WARNING]
> **WIP — early beta.** The `kuk` CLI is usable and stable on its core path
> (compare two PNGs, get score + diff). The bundled `kuk-compare` skill is
> still in flux — flags, prompts, and the Figma-MCP gating logic may shift
> between minor versions. Expect breaking changes until v1.0.

CLI that compares two webpage screenshots — design vs implementation — and produces
a similarity score plus a red-overlay diff PNG highlighting the deltas.

Built around the [`image-compare`](https://docs.rs/image-compare) crate using its
hybrid algorithm (MSSIM on luma + RMS on chroma + alpha), tuned for screenshots
with anti-aliased text.

## Status

v0.2.0 — renamed from `peep` to `kuk` (name conflict). Output format selector (`--format human|json|toon`), dimension reporting on every run, distinct exit code on dimension mismatch. Deferred for later: TOML config, multiple algorithms, side-by-side output, anti-aliasing toggle.

## Usage

```sh
kuk design.png impl.png
# kuk
#   design.png  1600x1200
#   impl.png    1600x1200  match
# score: 0.9958 (99.58% similar)
# diff:  diff.png
```

Flags:

- `--output <path>` — where to write the diff PNG (default: `diff.png`)
- `--threshold <f64>` — minimum acceptable similarity, range `[0, 1]` (default: `0.99`; `1.0` = identical)
- `--gain <f32>` — visibility gain on the per-pixel diff before clamp (default: `4.0`; higher = exaggerate small differences)
- `--fail` — exit 1 when `score < threshold` (for CI)
- `--format <human|json|toon>` — output format (default: `human`)
- `--no-diff` — skip writing the diff image

Exit codes: `0` ok, `1` threshold breach (with `--fail`), `2` generic error, `3` dimension mismatch.

### Output formats

- `human` — multi-line text with a dims block, similarity score, and diff path.
- `json` — single compact JSON line with `a`, `b`, `dims_match`, `score`, `passed`, `diff_path`.
- `toon` — TOON encoding (token-efficient) with a `sources[2]{label,path,width,height}` array and scalar fields. Intended for LLM/agent consumption.

On dimension mismatch (exit 3), the same format conventions apply: `human` prints a human-readable mismatch report; `json` and `toon` emit structured payloads with `dims_match: false`, a `delta`, and `error: dimension_mismatch`.

## Skill

`skills/kuk-compare/` is a bundled Claude Code skill that drives the full design-vs-implementation workflow: Figma desktop MCP is **required** — the skill gates on both MCP tools being loaded and the correct file being open before any REST call. It uses MCP to navigate and visually confirm the target node, then calls `skills/kuk-compare/scripts/figma-fetch.sh` to download the design PNG at `--scale 2` (Figma default), then captures the implementation via `agent-browser` at DPR=2 and runs `kuk`.

Enable globally (one-time):

```sh
ln -s "$PWD/skills/kuk-compare" ~/.claude/skills/kuk-compare
```

Requirements: `FIGMA_TOKEN` env var (scope **File content: Read**, get one at <https://www.figma.com/settings>), plus `curl` and `jq`.

`scripts/figma-fetch.sh` is also usable standalone:

```sh
export FIGMA_TOKEN=figd_...
DESIGN=$(skills/kuk-compare/scripts/figma-fetch.sh <fileKey> <nodeId>)
kuk "$DESIGN" impl.png --format toon
```

Flags: `--scale N` (0.01–4.0, default `2` — the skill always uses the default), `--format png|jpg|svg|pdf` (default `png`), `--absolute|--no-absolute` (sets `use_absolute_bounds`, default on), `--out PATH|-` (default: auto-generated path under `$TMPDIR`; `-` streams bytes to stdout).

## License

MIT
