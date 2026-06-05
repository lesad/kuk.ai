---
name: peep-compare
description: Use when visually comparing a Figma design against an implementation screenshot using the peep CLI. Uses Figma desktop MCP for navigation and mandatory visual validation, then the REST API (via $SKILL_DIR/scripts/figma-fetch.sh) for the actual PNG download — REST is only called after user confirms the right node. Performs autonomous diff analysis on failure before asking the user.
---

# Peep Visual Comparison

## Overview

This skill compares a Figma design frame against a browser implementation screenshot using the `peep` CLI. It produces a similarity score and a red-overlay diff image. On failure, Claude analyzes the diff autonomously and only asks the user when the findings are ambiguous.

**Required tools:**
- `peep` — similarity scoring and diff generation
- Chrome (or any Chromium-family browser: Edge, Arc, Brave, Vivaldi) with DevTools (built-in) — used for impl capture in Step 2
- `sips` — built-in on macOS; used only on dimension mismatch to resize externally
- Figma desktop MCP (`mcp__figma-desktop__*`) — **navigation and visual confirmation only**; not used to deliver pixels to peep
- `$SKILL_DIR/scripts/figma-fetch.sh` — REST helper that downloads a Figma node as PNG. Lives in the peep-rs repo at `$SKILL_DIR/scripts/figma-fetch.sh`.
- `curl`, `jq` — required by `figma-fetch.sh`
- `FIGMA_TOKEN` env var — Figma personal access token with `file_content:read` scope. Set once: `set -Ux FIGMA_TOKEN figd_...` (fish) or export in your shell rc.

> **Note — MCP image content blocks are vision-only.** Calling `mcp__figma-desktop__get_screenshot` returns a rendered image that the assistant can *see*, but the underlying base64 is not exposed as text and is not cached to disk. Do not attempt to forward MCP image bytes to peep — always use `$SKILL_DIR/scripts/figma-fetch.sh` for the actual capture.

---

## Workflow

### Step 1 — Capture the design (MCP navigation + REST fetch)

The design capture is a hybrid: cheap Figma desktop MCP for navigation **and validation**, then a one-shot REST call for the bytes. The REST call is the single moment we burn API quota — never make it speculatively.

#### Branch A — Figma desktop MCP is available (`mcp__figma-desktop__*` tools loaded)

This is the strongly preferred path. Use MCP for everything until the target node is **visually confirmed** by the user; only then call REST.

1. **Get the Figma URL** from the user. Parse `fileKey` and the `node-id` query param. URL format: `https://www.figma.com/design/<fileKey>/<name>?node-id=1-2`. If the URL is the `branch` form (`/design/<fileKey>/branch/<branchKey>/<name>`), use the `branchKey` as the fileKey for the API call.

2. **Resolve nodeId** (only if the user gave a frame name instead of an ID):
   - Call `mcp__figma-desktop__get_metadata` with the page id (or no nodeId to list pages) and walk the returned XML tree to find the node whose `name` matches what the user asked for. The MCP rate budget is independent of REST, so multiple discovery calls are free.

3. **Mandatory visual confirm via MCP — do NOT skip:**
   - Call `mcp__figma-desktop__get_screenshot` with the candidate nodeId. The returned image is for *your* visual inspection — it never becomes the peep input (base64 is not extractable from MCP image content blocks).
   - Show the user what you see and ask: "is this the right frame?" Be explicit about node name and id.
   - **If the user says NO (or the user disagrees with your interpretation):** go to step 4. Do NOT proceed to REST. Each wrong REST call burns Tier-1 quota (~10–20 req/min) and produces a useless PNG.

4. **Recover via metadata navigation** (loop until confirmed):
   - Call `mcp__figma-desktop__get_metadata` with the *parent* of the rejected node (or with no nodeId to inspect the whole page) and present candidate sibling/child names to the user, or re-walk the tree using the user's clarification.
   - For each new candidate, **repeat step 3** (MCP screenshot + user confirm). MCP is cheap; iterate until the user explicitly says "yes, that's the one."

5. **Only after explicit user confirmation, fetch the PNG via REST:**
   ```bash
   DESIGN=$($SKILL_DIR/scripts/figma-fetch.sh "$FILE_KEY" "$NODE_ID")
   ```
   The script writes to `${TMPDIR:-/tmp}/figma-<fileKey>-<nodeId>-2x.png` by default and prints the path on stdout. Use `--scale N`, `--format png|jpg|svg`, `--no-absolute`, or `--out PATH` for non-default behavior. Re-running with the same args overwrites the same path — that's the design's stable identifier.

#### Branch B — Figma desktop MCP is NOT available

If `mcp__figma-desktop__*` tools aren't loaded (no Figma desktop running, MCP server not reachable, etc.), you cannot pre-validate visually. Proceed cautiously:

1. Same URL parsing as Branch A step 1.
2. **If the user gave a frame name instead of a nodeId, STOP** and ask the user to open Figma and copy the node-id from the URL bar (right-click frame → Copy link). Do not guess — wrong fetches cost REST quota.
3. With an unambiguous `<fileKey>, <nodeId>` pair, call the REST helper as in Branch A step 5.

#### Common — token errors

If `FIGMA_TOKEN` is unset, the script exits 3 with a clear stderr message. Tell the user to generate one at <https://www.figma.com/settings> (Security → Personal access tokens → scope `File content: Read`) and export it as `FIGMA_TOKEN`. Then re-run.

### Step 2 — Capture the implementation

You already have the target dims from Step 1 — the Figma frame's logical size times the `--scale` you fetched at (default 2). Keep those in hand.

Use Chrome's built-in DevTools node screenshot. Pixel-perfect at the page DPR. Works against your already-open tab.

1. Open the page in Chrome at **100% zoom** (Cmd+0). Browser zoom distorts capture scale.
2. Right-click the target element → **Inspect** (F12 / Cmd+Opt+I).
3. In the Elements panel, right-click the highlighted DOM node → **Capture node screenshot**.
4. The PNG saves to `~/Downloads/`. Grab the latest:

   ```bash
   IMPL=$(ls -t ~/Downloads/*.png | head -1)
   cp "$IMPL" /tmp/impl.png
   ```

5. Run peep with TOON output (token-efficient for agent context):

   ```bash
   peep "$DESIGN" /tmp/impl.png --format toon
   ```

   - `dims_match: true` (exit 0) → read score, proceed to Step 3.
   - `dims_match: false` (exit 3) → read the `delta` block. If both `width` and `height` deltas are under ~5% of the design dims, resize externally:

     ```bash
     sips -z <design.height> <design.width> /tmp/impl.png --out /tmp/impl.png
     ```

     then rerun peep. If any delta is ≥5%, re-capture rather than distort.

**Full-page capture variant.** If you used "Capture full size screenshot" or "Capture screenshot" (viewport) instead of node-level, the result will rarely match the Figma frame. Either re-fetch the design at a matching `--scale` (`tools/figma-fetch.sh <key> <id> --scale 1` for DPR=1, etc.) or run the `sips` resize on whichever side is bigger.

### Step 3 — Run comparison

```bash
peep /tmp/design.png /tmp/impl.png --format toon
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
peep design.png impl.png --threshold 0.99 --fail --format json
```

Exit codes: `0` = pass, `1` = threshold breach, `2` = error, `3` = dimension mismatch.

Use `--gain` to amplify subtle differences in CI reports:
```bash
peep design.png impl.png --threshold 0.99 --fail --format json --gain 8
```

---

## Flags Reference

| Flag | Default | Purpose |
|------|---------|---------|
| `--output <path>` | `diff.png` | Diff PNG output path |
| `--threshold <f64>` | `0.99` | Minimum acceptable similarity (0 = no match, 1 = identical) |
| `--gain <f32>` | `4.0` | Diff visibility multiplier — higher values amplify subtle differences |
| `--fail` | off | Exit with code 1 when score < threshold |
| `--format <human\|json\|toon>` | `human` | Output format. `json` / `toon` for machine/agent consumption |
| `--no-diff` | off | Skip writing the diff image |
