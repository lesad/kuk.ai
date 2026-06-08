---
name: kuk-compare
description: Use when visually comparing a Figma design against an implementation screenshot using the kuk CLI. Uses Figma desktop MCP for navigation and mandatory visual validation, then the REST API (via $SKILL_DIR/scripts/figma-fetch.sh) for the design PNG. Default impl capture is the agent-browser skill — viewport-precise, element-by-CSS-selector, supports CSS injection for diagnostic loops; Chrome DevTools is the manual fallback. Performs autonomous diff analysis on failure before asking the user.
---

# Peep Visual Comparison

## Overview

This skill compares a Figma design frame against a browser implementation screenshot using the `kuk` CLI. It produces a similarity score and a red-overlay diff image. On failure, Claude analyzes the diff autonomously and only asks the user when the findings are ambiguous.

**Required tools:**
- `peep` — similarity scoring and diff generation
- `agent-browser` skill — **default impl-capture path** in Step 2. Headless browser with viewport control, element-by-selector screenshots, and CSS injection for diagnostic loops
- Chrome (or any Chromium-family browser: Edge, Arc, Brave, Vivaldi) with DevTools (built-in) — manual fallback for Step 2 when `agent-browser` can't reach the page (auth walls, MFA, internal-only allowlists)
- `sips` — built-in on macOS; last resort for known-margin crop (`sips -c`) only; never `sips -z` (resize distorts pixel-accurate comparison)
- Figma desktop MCP (`mcp__figma-desktop__*`) — **navigation and visual confirmation only**; not used to deliver pixels to kuk
- `$SKILL_DIR/scripts/figma-fetch.sh` — REST helper that downloads a Figma node as PNG. Lives in the kuk.ai repo at `$SKILL_DIR/scripts/figma-fetch.sh`.
- `curl`, `jq` — required by `figma-fetch.sh`
- `FIGMA_TOKEN` env var — Figma personal access token with `file_content:read` scope. Set once: `set -Ux FIGMA_TOKEN figd_...` (fish) or export in your shell rc.

> **Note — MCP image content blocks are vision-only.** Calling `mcp__figma-desktop__get_screenshot` returns a rendered image that the assistant can *see*, but the underlying base64 is not exposed as text and is not cached to disk. Do not attempt to forward MCP image bytes to kuk — always use `$SKILL_DIR/scripts/figma-fetch.sh` for the actual capture.

---

## Workflow

### Step 1 — Capture the design (MCP navigation + REST fetch)

The design capture is a hybrid: Figma desktop MCP for navigation **and visual confirmation**, then a one-shot REST call for the bytes. The REST call burns Tier-1 API quota — never call it speculatively, and **never before the correct node is visually confirmed via MCP**.

> **Prerequisites — verify before doing anything else:**
> 1. Confirm `mcp__figma-desktop__*` tools are loaded. If not → **stop**: "Open Figma desktop and ensure the MCP server is running."
> 2. Call `mcp__figma-desktop__get_metadata` with the target `fileKey` (no `nodeId` — file root is enough). If it fails or returns a different file → **stop**: "Open file `<fileKey>` in Figma desktop, then retry."
>
> Both checks must pass. MCP is **required** — there is no fallback REST-only path.

Use MCP for everything until the target node is **visually confirmed** by the user; only then call REST.

1. **Get the Figma URL** from the user. Parse `fileKey` and the `node-id` query param. URL format: `https://www.figma.com/design/<fileKey>/<name>?node-id=1-2`. If the URL is the `branch` form (`/design/<fileKey>/branch/<branchKey>/<name>`), use the `branchKey` as the fileKey for the API call.

2. **Resolve nodeId** (only if the user gave a frame name instead of an ID):
   - Call `mcp__figma-desktop__get_metadata` with the page id (or no nodeId to list pages) and walk the returned XML tree to find the node whose `name` matches what the user asked for. The MCP rate budget is independent of REST, so multiple discovery calls are free.

3. **Mandatory visual confirm via MCP — do NOT skip:**
   - Call `mcp__figma-desktop__get_screenshot` with the candidate nodeId. The returned image is for *your* visual inspection — it never becomes the kuk input (base64 is not extractable from MCP image content blocks).
   - Show the user what you see and ask: "is this the right frame?" Be explicit about node name and id.
   - **If the user says NO (or the user disagrees with your interpretation):** go to step 4. Do NOT proceed to REST. Each wrong REST call burns Tier-1 quota (~10–20 req/min) and produces a useless PNG.

4. **Recover via metadata navigation** (loop until confirmed):
   - Call `mcp__figma-desktop__get_metadata` with the *parent* of the rejected node (or with no nodeId to inspect the whole page) and present candidate sibling/child names to the user, or re-walk the tree using the user's clarification.
   - For each new candidate, **repeat step 3** (MCP screenshot + user confirm). MCP is cheap; iterate until the user explicitly says "yes, that's the one."

5. **Only after explicit user confirmation, fetch the PNG via REST:**
   ```bash
   DESIGN=$($SKILL_DIR/scripts/figma-fetch.sh "$FILE_KEY" "$NODE_ID")
   ```
   The script writes to `${TMPDIR:-/tmp}/figma-<fileKey>-<nodeId>-2x.png` and prints the path on stdout. Always use the default scale (2) — it is the Figma export default and matches DPR=2 in agent-browser. Use `--format png|jpg|svg`, `--no-absolute`, or `--out PATH` for non-default behavior. Re-running with the same args overwrites the same path — that's the design's stable identifier.

   **Render timeout** — if `figma-fetch.sh` returns `{"status":400,"err":"Render timeout"}`:
   1. Call `mcp__figma-desktop__get_metadata` on the target node, pick a logical child (e.g. the content frame, not the outer wrapper), retry `figma-fetch.sh` with that child ID.
   2. If all sub-nodes also timeout → ask the user to manually export from Figma desktop (right-click → Export → PNG 2×) and place the file at `/tmp/design.png`.
   3. Do **not** retry the same node at different scales — a complex frame times out at both scale 1 and scale 2.

#### Common — token errors

If `FIGMA_TOKEN` is unset, the script exits 3 with a clear stderr message. Tell the user to generate one at <https://www.figma.com/settings> (Security → Personal access tokens → scope `File content: Read`) and export it as `FIGMA_TOKEN`. Then re-run.

To verify a token before fetching — a `file_content:read`-scoped token returns 403 on `/v1/me`; use the file-node endpoint instead:

```bash
curl -sS -o /dev/null -w "%{http_code}" \
  -H "X-Figma-Token: $FIGMA_TOKEN" \
  "https://api.figma.com/v1/files/<fileKey>/nodes?ids=0:1"
# 200 = valid; 403 = wrong token or missing file_content:read scope
```

### Step 2 — Capture the implementation

Always fetch the design at `--scale 2` (Figma default). The impl capture must match: DPR=2 in agent-browser, or node screenshot on a retina display in Branch B. Step 2 ends with a PNG at `/tmp/impl.png` and nothing else.

Two paths. **Branch A (`agent-browser`) is the default and strongly preferred** — programmable, viewport-precise, element-scoped by CSS selector, and supports CSS injection for diagnostic loops. **Branch B is a manual fallback** for when `agent-browser` can't reach the page (auth wall, MFA, IP allowlist, Electron app, dev environment behind a tunnel that only the user has).

#### Branch A — agent-browser skill (default)

Invoke the `agent-browser` skill. Copy-paste capture sequence:

```bash
# 1. Set viewport before navigating — DPR=2 matches Figma's default --scale 2 export
#    CSS layout stays at W×H; screenshots render at W×2 pixels wide
#    Width matters for layout; height can be loose (element capture clips to content height)
agent-browser set viewport <FigmaLogicalWidth> 960 2

# 2. Navigate (if auth needed, run `agent-browser auth login` first)
agent-browser open <impl-url>
agent-browser wait --load networkidle

# 3. Kill animations — use --stdin heredoc to avoid shell quoting issues
agent-browser eval --stdin <<'EVALEOF'
const s = document.createElement("style");
s.textContent = "* { transition: none !important; animation: none !important; }";
document.head.appendChild(s);
EVALEOF

# 4. Capture element directly — output pixel dims = logical dims × DPR; no full-page + crop needed
agent-browser screenshot "[data-testid='target-element']" /tmp/impl.png
```

If the page needs auth and `agent-browser auth login` doesn't cover it, fall back to Branch B.

Selector preference (most to least stable):
- `data-testid` attributes — stable across refactors, the project convention if it exists.
- Semantic roles (`role="navigation"`, `role="button"[name="Save"]`) — survive class renames.
- Unique IDs (`#user-profile-card`) — fine if the team is disciplined about uniqueness.
- Avoid auto-generated class hashes (`.css-1a2b3c`, `._abcd_123`) — they break on every build.

##### CSS injection — diagnostic and stabilisation tool

`agent-browser` can inject CSS into the page before capture. Use this for two distinct purposes:

**1. Stabilise the impl before capture** (reduces false-positive diffs):
- Kill animations + transitions so the capture is deterministic: `* { transition: none !important; animation: none !important; }`
- Pin draggable / sortable elements to a known position: `.drag-target { transform: none !important; }`
- Hide loading skeletons that flicker into view: `.skeleton-loader { display: none !important; }`

**2. Bisect a bug** (peep flagged a region, you want to know which delta is causing it):
- **Hide elements not yet implemented in the design** so they don't dominate the diff: `[data-feature="beta"] { display: none !important; }`. The design doesn't show that button — don't penalise the impl for it.
- **Try a fix in-place** — inject `padding`, `font-size`, `color` adjustments, re-capture, re-run kuk, watch the score move. Faster than rebuilding the app between hypotheses.
- **Force the suspected state** — e.g. add `.button.hover-state` styles directly so you can compare against a hover-state design without driving real input events.

**Always report any injected CSS to the user** in your final summary. They need to know that the diff score reflects the *tweaked* impl, not what would ship. If the score is only good with three rules of injected CSS, that's three real bugs to file, not a passing test.

#### Branch B — Chrome DevTools (manual fallback)

Use this only when Branch A can't reach the page or when the user explicitly asks for manual capture. Pixel-perfect at the page DPR, works against your already-open tab.

1. Open the page in Chrome at **100% zoom** (Cmd+0). Browser zoom distorts capture scale.
2. Right-click the target element → **Inspect** (F12 / Cmd+Opt+I).
3. In the Elements panel, right-click the highlighted DOM node → **Capture node screenshot**.
4. The PNG saves to `~/Downloads/`. Grab the latest:

   ```bash
   IMPL=$(ls -t ~/Downloads/*.png | head -1)
   cp "$IMPL" /tmp/impl.png
   ```

**Full-page capture variant.** If you used "Capture full size screenshot" or "Capture screenshot" (viewport) instead of node-level, re-capture using "Capture node screenshot" — it renders at the page DPR and on a retina display matches the `--scale 2` design fetch. Do not use `sips -z` to resize either side.

### Step 2.5 — Pre-flight sanity check (smoke test)

Before invoking kuk, eyeball both inputs yourself. kuk computes MSSIM + chroma diff, which is wasted compute when the two images are obviously of different content. Catch wildly-wrong impl captures up-front so you can re-capture rather than report a near-zero score.

Read both PNGs into your vision context:

```bash
# Confirm both files exist and are non-empty before reading
ls -l "$DESIGN" /tmp/impl.png
```

Then use the **Read** tool on each path. Vision-inspect for obvious smell tests — these are red flags, not exhaustive:

- **Wildly different content** — design shows a dashboard, impl shows a login page. The user captured the wrong route or wrong element.
- **Different theme** — design is dark mode, impl is light mode (or vice versa). User has the wrong theme toggle.
- **Different viewport class** — design is a 1440px desktop frame, impl is a 375px mobile slice. User captured at the wrong breakpoint.
- **Different language / locale** — design is in English, impl is in Czech. Browser locale mismatch.
- **Empty / blank / loading state** — impl PNG is a solid color, skeleton, or "Loading…" spinner. User captured before the page settled.
- **Massive layout offset** — element is in roughly the right place but a whole header / sidebar is missing on one side. User cropped a different region than the design covers.

**Decision:**

- **All sanity checks pass** → proceed to Step 3 (run kuk). Do not manually verify image dimensions — peep reports dimension mismatches via exit code 3.
- **One or more red flags** → STOP. Do not run kuk. Report the specific mismatch to the user (e.g., "the impl looks like a mobile capture but the design is the desktop frame — can you re-capture at 1440px?") and wait for a re-capture before continuing. Peep would produce a near-zero score and a fully red diff image that adds no diagnostic value beyond what you can already see.

**If peep exits with code 3 (dimension mismatch):** fix via `agent-browser set viewport` width adjustment and/or CSS `width` override on the element — re-capture, re-run. Do **not** use `sips -z` (resize/scale — distorts the image and defeats pixel-accurate comparison). `sips -c H W` (crop only) is a last resort only after confirming the overflow is a known extra margin, not a real visual bug. A height mismatch that isn't from a known CSS overflow is a real finding — report it, don't hide it by cropping.

This step is cheap (two image reads) and saves cycles when the human-loop part of the workflow has gone off the rails.

### Step 3 — Run comparison

```bash
kuk /tmp/design.png /tmp/impl.png --format toon
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
kuk design.png impl.png --threshold 0.99 --fail --format json
```

Exit codes: `0` = pass, `1` = threshold breach, `2` = error, `3` = dimension mismatch.

Use `--gain` to amplify subtle differences in CI reports:
```bash
kuk design.png impl.png --threshold 0.99 --fail --format json --gain 8
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
