---
name: peep-compare
description: Use when visually comparing a Figma design against an implementation screenshot using the peep CLI. Uses Figma desktop MCP for navigation and the REST API (via $SKILL_DIR/scripts/figma-fetch.sh) for the actual PNG download. Performs autonomous diff analysis on failure before asking the user.
---

# Peep Visual Comparison

## Overview

This skill compares a Figma design frame against a browser implementation screenshot using the `peep` CLI. It produces a similarity score and a red-overlay diff image. On failure, Claude analyzes the diff autonomously and only asks the user when the findings are ambiguous.

**Required tools:**
- `peep` — similarity scoring and diff generation
- `pngpaste` — saves clipboard images to disk (used in Step 2)
- Figma desktop MCP (`mcp__figma-desktop__*`) — **navigation and visual confirmation only**; not used to deliver pixels to peep
- `$SKILL_DIR/scripts/figma-fetch.sh` — REST helper that downloads a Figma node as PNG. Lives in the peep-rs repo at `$SKILL_DIR/scripts/figma-fetch.sh`.
- `curl`, `jq` — required by `figma-fetch.sh`
- `FIGMA_TOKEN` env var — Figma personal access token with `file_content:read` scope. Set once: `set -Ux FIGMA_TOKEN figd_...` (fish) or export in your shell rc.

> **Note — MCP image content blocks are vision-only.** Calling `mcp__figma-desktop__get_screenshot` returns a rendered image that the assistant can *see*, but the underlying base64 is not exposed as text and is not cached to disk. Do not attempt to forward MCP image bytes to peep — always use `$SKILL_DIR/scripts/figma-fetch.sh` for the actual capture.

---

## Workflow

### Step 1 — Capture the design (MCP navigation + REST fetch)

The design capture is a hybrid: cheap Figma desktop MCP for navigation, then a one-shot REST call for the bytes.

1. **Get the Figma URL** from the user. Parse `fileKey` and the `node-id` query param. The URL format is `https://www.figma.com/design/<fileKey>/<name>?node-id=1-2`. If the URL is the `branch` form (`/design/<fileKey>/branch/<branchKey>/<name>`), use the `branchKey` as the fileKey for the API call.
2. **Resolve nodeId** (only if the user gave a frame name instead of an ID):
   - Call `mcp__figma-desktop__get_metadata` with the page id (or no nodeId to list pages) and walk the returned XML tree to find the node whose `name` matches what the user asked for. The MCP rate budget is independent of REST, so multiple discovery calls are fine.
3. **Visual confirm** (only if it's ambiguous which node the user meant):
   - Call `mcp__figma-desktop__get_screenshot` with the candidate nodeId and ask the user "this one?" The image you receive is for *your* visual inspection — it never becomes the peep input.
4. **Fetch the PNG via REST:**
   ```bash
   DESIGN=$($SKILL_DIR/scripts/figma-fetch.sh "$FILE_KEY" "$NODE_ID")
   ```
   The script writes to `${TMPDIR:-/tmp}/figma-<fileKey>-<nodeId>-2x.png` by default and prints the path on stdout. Use `--scale N`, `--format png|jpg|svg`, `--no-absolute`, or `--out PATH` for non-default behavior. Re-running with the same args overwrites the same path — that's the design's stable identifier.

If `FIGMA_TOKEN` is unset, the script exits 3 with a clear stderr message. Tell the user to generate one at <https://www.figma.com/settings> (Security → Personal access tokens → scope `File content: Read`) and export it as `FIGMA_TOKEN`. Then re-run.

### Step 2 — Capture the implementation

Ask the user to take a screenshot of the browser implementation and copy it to the clipboard, then:

```bash
pngpaste /tmp/impl.png
```

### Step 3 — Run comparison

```bash
peep /tmp/design.png /tmp/impl.png --json
```

---

## Interpreting Results

| Score | Verdict | Action |
|-------|---------|--------|
| 1.0 | Pixel-perfect | Report and finish |
| ≥ threshold (default 0.99) | Pass | Report score and finish |
| < threshold | Fail | Perform autonomous diff analysis (see below) |

---

## Autonomous Diff Analysis (on failure)

When the score falls below the threshold, **do not immediately ask the user**. Instead, analyze the diff image yourself:

1. **Read `diff.png` exactly once.** The image is 4K — extract all findings in a single read. Do not re-read it.
2. **Identify red zones.** The red overlay marks pixel-level differences. Cluster the red areas into distinct regions by position (e.g., top navigation, center button group, footer).
3. **Map each zone to a likely cause:**
   - Large contiguous red block → structural difference: layout, spacing, or sizing
   - Fine red speckle on text → typography: font weight, size, or rendering
   - Red fringe on element edges → border, shadow, or anti-aliasing difference
   - Isolated red in a control → color, opacity, or interaction state mismatch
4. **Write a concise summary** of each affected area and its probable cause.
5. **Ask the user only if the diff is ambiguous** — for example, when red is uniformly faint and widespread with no clear cluster pattern.

### Example diff report

> Diff analysis (score: 0.962):
>
> - **Top navigation bar** (top ~15%): large red band — likely a height or padding difference.
> - **Button group** (center-right): scattered red on button labels — possible font-weight mismatch.
> - **Footer** (bottom 8%): faint uniform red — may be a background color drift.
>
> Overall: spacing and typography issues. Recommend reviewing nav height and button label styles before re-running.

---

## CI Path

```bash
peep design.png impl.png --threshold 0.99 --fail --json
```

Exit codes: `0` = pass, `1` = threshold breach, `2` = error.

Use `--gain` to amplify subtle differences in CI reports:
```bash
peep design.png impl.png --threshold 0.99 --fail --json --gain 8
```

---

## Flags Reference

| Flag | Default | Purpose |
|------|---------|---------|
| `--output <path>` | `diff.png` | Diff PNG output path |
| `--threshold <f64>` | `0.99` | Minimum acceptable similarity (0 = no match, 1 = identical) |
| `--gain <f32>` | `4.0` | Diff visibility multiplier — higher values amplify subtle differences |
| `--fail` | off | Exit with code 1 when score < threshold |
| `--json` | off | Machine-readable output on stdout |
| `--no-diff` | off | Skip writing the diff image |
