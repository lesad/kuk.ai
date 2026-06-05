# Impl-capture flow + dimension reporting + `--format` flag

Date: 2026-06-05
Status: draft (pending implementation)

## Context

`peep-rs` v0.1.0 has been working: compares two PNGs, prints a score, writes a red-overlay diff. The `peep-compare` skill has Step 1 (Figma fetch via REST) and Step 3 (run peep) already settled by a parallel work stream. Step 2 — implementation capture — is the gap.

Earlier handoff proposed a three-tier impl-capture ladder (`screencapture -i`, `pngpaste`, and a Python CDP element-screenshot script). User rejected all three:

- **Python CDP script** — requires relaunching Chrome with `--remote-debugging-port=9222`, violating "hook into existing session". Adds Python + `websocket-client` dep. Chrome extension model also forbids external invocation of installed extensions, so no extension-based shortcut exists either.
- **`screencapture -i`** and **`pngpaste`** — work but lose pixel-level precision against a known Figma frame; the impl crop is eyeballed by the user.
- **cmux browser pane** — stripped WebKit, no DevTools, no node screenshot. Dead.

The user picked **Chrome (or any Chromium-family) DevTools "Capture node screenshot"** as the sole capture mechanism. Pixel-perfect at the page's device pixel ratio, runs against the already-open tab, two clicks, no install. Output lands in `~/Downloads/`.

That solves Step 2 capture, but exposes a second problem: any time the impl PNG's dimensions differ from the design PNG (DPR mismatch, full-page capture, slight zoom, etc.), `peep` fails opaquely. `PeepError::DimMismatch` already exists in the binary; it just isn't surfaced in a structured, format-aware way, and shares exit code 2 with generic errors.

This spec covers both:

1. **Skill Step 2** rewritten as pure instructions using DevTools node screenshot.
2. **Binary upgrade** to report both sides' dimensions in every successful run, surface dimension mismatch as a distinct recoverable error (exit 3), and support three output formats — `human` (default), `json`, `toon` — via a new `--format` flag that replaces the existing `--json` bool.

Resize / image manipulation stays out of the binary. Mismatch is reported; the agent or user resolves it externally with `sips`, re-capture, or a different Figma `--scale`.

## Goals

- Step 2 of the `peep-compare` skill becomes a self-contained set of instructions, with no new bundled tool or script.
- `peep` always echoes the dimensions of both input images in its output, in whichever format the user asked for.
- Dimension mismatch is a structured, recoverable error: distinct exit code, structured payload in JSON/TOON, actionable suggestion in human output.
- Output format is selectable: `--format human|json|toon`, default `human`.
- No new runtime dependencies. TOON encoding is hand-rolled for our fixed, small schema.
- Rust binary version bumps `0.1.0 → 0.2.0`.

## Non-goals

- Resize inside the binary. The agent picks `sips -z`, re-capture, or a different Figma scale.
- Backwards-compatibility shim for the removed `--json` flag. v0.x; clean break.
- Touching Step 1 (`tools/figma-fetch.sh`), Step 3, or the underlying `image-compare` pipeline.
- Capture-side tooling: no `tools/screenshot-element.py`, no `pngpaste`, no `screencapture -i` documentation. Skill instructs the user to use the browser's built-in DevTools.
- TOON encoder crate dependency. Schema is fixed and small; hand-roll.

## CLI surface

```
peep <design> <impl> [--threshold F64] [--gain F32] [--fail]
                     [--output PATH] [--no-diff]
                     [--format human|json|toon]
```

`--format` accepts only the lowercase variants. Clap's `ValueEnum` with `rename_all = "lower"` is the implementation hook; the internal Rust enum stays CamelCase per convention.

`--json` (bool) is **removed**, not aliased. Anyone scripting against it migrates to `--format json`.

## Output shapes

### Human (default)

Success:

```
peep
  design.png  1600x1200
  impl.png    1600x1200  match
score: 0.9958 (99.58% similar)
diff:  diff.png
```

Mismatch (no compare runs, no diff written):

```
peep: dimension mismatch
  design.png  1600x1200
  impl.png    1620x1198  (+20, -2)
resize externally (e.g. sips -z 1200 1600 impl.png --out impl.png)
or re-capture if delta exceeds ~5%.
```

The mismatch report goes to **stdout** (was stderr in v0.1.0) so that scripts driving `peep` can read it from the same stream as the success report.

### JSON

Success:

```json
{
  "a": { "path": "design.png", "width": 1600, "height": 1200 },
  "b": { "path": "impl.png", "width": 1600, "height": 1200 },
  "dims_match": true,
  "score": 0.9958,
  "threshold": 0.99,
  "passed": true,
  "diff_path": "diff.png"
}
```

Mismatch (exit 3):

```json
{
  "a": { "path": "design.png", "width": 1600, "height": 1200 },
  "b": { "path": "impl.png", "width": 1620, "height": 1198 },
  "dims_match": false,
  "delta": { "width": 20, "height": -2 },
  "error": "dimension_mismatch"
}
```

Output is a single compact line terminated by `\n` (unchanged from v0.1.0 JSON behaviour).

`delta.width` is `b.width - a.width`. `delta.height` is `b.height - a.height`. Both are signed.

### TOON

Success:

```toon
sources[2]{label,path,width,height}:
  a,design.png,1600,1200
  b,impl.png,1600,1200
dims_match: true
score: 0.9958
threshold: 0.99
passed: true
diff_path: diff.png
```

Mismatch (exit 3):

```toon
sources[2]{label,path,width,height}:
  a,design.png,1600,1200
  b,impl.png,1620,1198
dims_match: false
delta:
  width: 20
  height: -2
error: dimension_mismatch
```

Hand-rolled formatter. Two-space indent. `[N]` count matches row count. Comma delimiter for the tabular `sources` array. No escaping needed for the values we emit (paths and integers); if a path ever contains a comma we wrap it in double quotes — out of scope here since paths come from CLI args and we don't need to defensively quote real-world inputs.

`diff_path` is omitted from JSON and TOON when `--no-diff` is set (matches current Report.diff_path serde behaviour).

## Error model

`PeepError::DimMismatch` already exists. Field names stay (`width_a/height_a/width_b/height_b`), but the surrounding plumbing changes:

- Renamed thiserror `#[error]` text is irrelevant — the error never gets stringified into stderr in the success/mismatch paths now; the renderer handles it.
- `main.rs` recognises `DimMismatch` and routes it to a structured renderer instead of the generic stderr path.
- All other `PeepError` variants still go through the generic stderr path with exit 2.

## Exit codes

| Code | Meaning | When |
|---|---|---|
| 0 | Success | Compare ran, no `--fail` breach |
| 1 | Threshold breach | `--fail` set and `score < threshold` |
| 2 | Generic error | `ImageLoad`, `DiffWrite`, `Compare` |
| 3 | Dimension mismatch | New. Recoverable. Distinct from 2 so callers can branch. |

## File-by-file plan

### `src/cli.rs`

- Define `OutputFormat { Human, Json, Toon }` enum, `derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)`, `#[clap(rename_all = "lower")]`.
- Replace `pub json: bool` with `pub format: OutputFormat` (default `OutputFormat::Human`).
- Update `tests/all_defaults_should_be_set_correctly` and `tests/all_flags_overridden_should_reflect_new_values` to assert `format` instead of `json`.

### `src/error.rs`

- No structural changes. `DimMismatch` already fits. The `#[error]` string stays for the generic stderr path that other code may still hit (e.g. tests / debug formatting), even though `main.rs` short-circuits it for the format-aware renderer.

### `src/report.rs`

- Replace single `width/height` with paired image info. Two options considered:
  - **Picked:** add fields `a: ImageInfo` and `b: ImageInfo` where `ImageInfo { path: PathBuf, width: u32, height: u32 }`. Drops the bare `width`/`height` fields. Aligns with the JSON/TOON output shape.
  - Rejected: keep `width/height` for the equal-dims path, only attach the pair on mismatch. Asymmetric; harder to write the human formatter.
- Add `dims_match: bool` (always `true` in a successful `Report` since mismatch never produces a `Report`).
- Add format-aware renderers: `to_human()` (current behavior + new dims block at top), `to_json()` (existing, expanded fields), `to_toon()` (new, hand-rolled).
- `Report::from_compare` now needs the `design_path` and `impl_path` to populate `a.path`/`b.path`. Threaded from `main.rs::run`.
- Existing `Serialize` derive keeps working for `to_json` once fields are added; field rename via `#[serde(rename)]` only if needed for the `a`/`b` keys (likely yes since Rust field names are `a`/`b`, which is fine).

### `src/compare.rs`

- `CompareResult` keeps its `width`/`height` (they're equal at this point — the function only returns Ok on dim match). No change here.
- `run()` already returns `PeepError::DimMismatch` on dim divergence. No change.

### `src/main.rs`

- `main()`'s error arm matches on `PeepError::DimMismatch` first and routes it to a new `print_mismatch(format, design_path, impl_path, dim_a, dim_b)` renderer that writes to **stdout** and returns `ExitCode::from(3)`. All other `PeepError` variants stay on the existing stderr + exit-2 path.
- `print_report` switches on `OutputFormat` instead of the `json` bool. Adds a `Toon` arm calling `report.to_toon()`.
- `run()` returns `Result<Report, PeepError>`; pass `args.design.clone()` and `args.implementation.clone()` into `Report::from_compare` so the report carries the paths.

### `tests/cli.rs` (integration tests)

Existing tests using `--json` are migrated to `--format json`. The `dimension_mismatch_should_exit_2_with_sizes_in_stderr` test is renamed and rewritten:

- `dimension_mismatch_human_should_exit_3_with_dims_on_stdout` — asserts exit 3 and stdout contains `dimension mismatch`, `4x4`, `8x8`.
- `dimension_mismatch_json_should_exit_3_with_structured_payload` — asserts exit 3, parses stdout as JSON, asserts `dims_match=false`, `a`/`b` dims, `delta`, `error="dimension_mismatch"`.
- `dimension_mismatch_toon_should_exit_3_with_structured_payload` — asserts exit 3, stdout contains `dims_match: false` and `error: dimension_mismatch` and the `sources[2]{...}` header.

Add:

- `identical_pngs_with_format_human_should_print_dims_block` — asserts the new dims block in human output (success path).
- `help_flag_should_show_format_option` — replaces `--json` assertion in the existing help test.

Drop the existing `--json` references from `help_flag_should_exit_0_and_show_all_flags`.

### `src/cli.rs` unit tests

Update both default and override tests to refer to `args.format` (`OutputFormat::Human` / `OutputFormat::Json`).

### `Cargo.toml`

Bump `version = "0.2.0"`. No dep changes.

### `CHANGELOG.md`

Under `[Unreleased]` (rename to `[0.2.0] - 2026-06-05` once cut):

- **Added** — `--format human|json|toon` flag selecting output format. Default `human`.
- **Added** — Dimension reporting in every output format. Successful runs echo both inputs' dimensions; dimension mismatch is reported with delta.
- **Added** — Exit code 3 for dimension mismatch (recoverable, distinct from generic error 2).
- **Added** — TOON output via the new `--format toon` for token-efficient agent consumption.
- **Changed** — Dimension mismatch report now goes to **stdout** in structured form (was stderr in v0.1.0). Same stream as the success report so callers parse once.
- **Removed** — `--json` flag. Replaced by `--format json`. No alias.
- **Changed** — `peep-compare` skill Step 2 rewritten: Chrome DevTools "Capture node screenshot" as the sole impl capture; latest `~/Downloads/*.png` copied to `/tmp/impl.png`; on exit 3, agent resolves with `sips` or re-capture.

### `README.md`

Update the Usage section's flags table: drop `--json`, add `--format`. Add a one-paragraph "Output formats" subsection demonstrating each. Update example output to show the new dims block.

### `~/.claude/skills/peep-compare/SKILL.md`

Rewrite Step 2 only. Drop `pngpaste` from the "Required tools" list; add "Chrome / Chromium-family browser with DevTools (built-in)". Keep Step 1 and Step 3 untouched (other agent's territory).

New Step 2 text:

```markdown
### Step 2 — Capture the implementation

You already have the target dims from Step 1 — the Figma frame's logical
size times the `--scale` you fetched at (default 2). Keep those in hand.

Use Chrome's built-in DevTools node screenshot. Pixel-perfect at the page
DPR. Works against your already-open tab.

1. Open the page in Chrome at **100% zoom** (Cmd+0). Browser zoom
   distorts capture scale.
2. Right-click the target element → **Inspect** (F12 / Cmd+Opt+I).
3. In the Elements panel, right-click the highlighted DOM node →
   **Capture node screenshot**.
4. The PNG saves to `~/Downloads/`. Grab the latest:
   ```bash
   IMPL=$(ls -t ~/Downloads/*.png | head -1)
   cp "$IMPL" /tmp/impl.png
   ```
5. Run peep with TOON output for compact agent context:
   ```bash
   peep "$DESIGN" /tmp/impl.png --format toon
   ```
   - `dims_match: true` (exit 0): read score, proceed to Step 3 / report.
   - `dims_match: false` (exit 3): read `delta`. If both `width` and
     `height` deltas are under ~5% of the design dims, resize externally:
     ```bash
     sips -z <design.h> <design.w> /tmp/impl.png --out /tmp/impl.png
     ```
     then rerun. If any delta is ≥5%, re-capture rather than distort.

**Full-page capture variant.** If you used "Capture full size screenshot"
or "Capture screenshot" (viewport) instead of node-level, the result will
rarely match the Figma frame. Either re-fetch the design at a matching
`--scale` (`tools/figma-fetch.sh <key> <id> --scale 1` for DPR=1, etc.)
or run the `sips` resize on whichever side is bigger.
```

The skill's "Required tools" list:
- **Add** — Chrome (or any Chromium-family: Edge, Arc, Brave, Vivaldi) with DevTools (built-in).
- **Add** — `sips` (built-in macOS, used only on mismatch).
- **Remove** — `pngpaste`.

## Tests to add

| Test | Asserts |
|---|---|
| `identical_pngs_should_exit_0_and_score_near_1_and_write_diff` | Already exists, no change. |
| `identical_pngs_with_format_human_should_print_dims_block` | New. Stdout contains both filenames + their dims + `match`. |
| `identical_pngs_with_format_json_should_produce_valid_json_with_a_b_blocks` | Replaces existing `--json` test. Asserts `a`, `b`, `dims_match=true`, `score`, `passed`. |
| `identical_pngs_with_format_toon_should_contain_sources_header_and_match` | New. Asserts `sources[2]{label,path,width,height}:`, `dims_match: true`, `score`. |
| `dimension_mismatch_human_should_exit_3_with_dims_on_stdout` | Replaces existing `dimension_mismatch_should_exit_2_with_sizes_in_stderr`. Exit 3. Stdout (not stderr) contains `dimension mismatch`, `4x4`, `8x8`. No diff written. |
| `dimension_mismatch_json_should_exit_3_with_structured_payload` | New. Exit 3. Parses stdout as JSON. `dims_match=false`, `error="dimension_mismatch"`, `a`/`b`/`delta` dims correct. |
| `dimension_mismatch_toon_should_exit_3_with_structured_payload` | New. Exit 3. Stdout contains TOON shape: `sources[2]{...}`, `dims_match: false`, `error: dimension_mismatch`. |
| `different_pngs_with_fail_and_format_json_should_exit_1_with_passed_false` | Migrate existing `different_pngs_with_fail_and_json_should_exit_1_with_passed_false` to `--format json`. |
| `different_pngs_without_fail_flag_should_exit_0_with_score_below_threshold` | Migrate to `--format json`. |
| `help_flag_should_exit_0_and_show_format_option` | Replaces `--json` substring check with `--format` and the literal `human`, `json`, `toon`. |
| `version_flag_should_exit_0_and_print_crate_version` | No change; will verify `0.2.0` after bump. |

Unit tests in `src/report.rs` and `src/cli.rs` adapt to the new `OutputFormat` enum and the new `a`/`b` fields.

## Rejected alternatives (recorded for posterity)

- **Tools script for impl capture** (`tools/screenshot-element.py`, CDP-based). Requires Chrome relaunch with debug port — kills "hook existing session". Adds Python + `websocket-client` dep. Replaced by browser-native DevTools.
- **`screencapture -i`**, **`pngpaste`** — lose pixel-perfect alignment with Figma frame; user eyeballs the crop.
- **Browser extensions** (Nimbus, FireShot, Awesome Screenshot) — Chrome security model blocks external invocation from cmux/terminal.
- **Firefox DevTools `:screenshot --selector`** — programmable, but user picked Chrome family.
- **cmux browser pane** — stripped WebKit, no DevTools, no node screenshot.
- **`tools/peep-dims.sh`** (proposed mid-discussion) — superseded by binary integration. Resolution checking is comparison-adjacent and belongs alongside `peep`'s normal output, not in shell glue.
- **`--resize-to a|b` flag** for in-binary resize. Out of scope; user wants resize to stay external (`sips`) so the binary keeps a single responsibility.
- **TOON encoder crate** dep. Schema is fixed and tiny; hand-roll, no dep cost.
- **Compat alias for `--json`**. Pre-1.0; clean break.
- **Bookmarklet + html2canvas**. Re-renders DOM in JS — fonts and anti-aliasing drift from native raster. False diff signal.

## Verification

After implementation:

1. `cargo build` — clean.
2. `cargo test` — all green. Migrated tests pass; new mismatch tests verify exit 3 + structured payload on each format.
3. `peep --help` — shows `--format` with the three lowercase values, no `--json`.
4. `peep --version` — reports `0.2.0`.
5. **Human format smoke:** `peep design.png impl.png` (matching dims) — output includes both filenames + dims block + `match`, then score + diff line.
6. **JSON format smoke:** `peep design.png impl.png --format json` — single line of JSON, contains `a`/`b`/`dims_match`/`score`/`diff_path`.
7. **TOON format smoke:** `peep design.png impl.png --format toon` — TOON output with `sources[2]{label,path,width,height}:` header.
8. **Mismatch exit code:** `peep 1600x1200.png 800x600.png` exits with code 3 regardless of `--format`.
9. **Mismatch goes to stdout, not stderr:** verified by running with `2>/dev/null` and checking that the structured report still appears on stdout.
10. **Skill end-to-end:** Run `peep-compare` against a real Figma frame + Chrome DevTools-captured node. Confirm `dims_match: true` path runs through to diff; deliberately capture at wrong zoom to trigger mismatch and confirm the agent receives the structured TOON report.

## Open questions

None. Pending user spec review then writing-plans.
