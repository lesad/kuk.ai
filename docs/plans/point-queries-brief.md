# Brief: point queries

Hand this to the agent that builds the feature. It is a brief, not a spec. The
first job is to turn it into a spec with Adrian, then a plan, then code.

## Prompt

You are working in `kuk.ai`, a Rust CLI (`kuk`) that compares a Figma design
PNG against a browser screenshot and writes a similarity score plus a
red-overlay diff PNG. Read `CLAUDE.md` first: it has the architecture, the exit
code contract, the CLI design rules and the skill invariants. Run
`kuk schema` to see the current commands, flags and payloads.

### The problem

When a comparison fails, the agent reads the red overlay with vision and
guesses which element is wrong. Vision is weak at this: it can say "red near
the top right", not "the `SaveButton` label is 2px too low". The browser
already knows what is under every pixel. We want the agent to ask about one
red point and get the stack of DOM elements and components under it.

### The goal

An agent that has a failing comparison can:

1. get a short list of the red regions, each with coordinates, and
2. ask about a point in one of them and receive the element and component
   stack at that point, straight from the live page.

"Done" is a real run: capture an implementation with `agent-browser`, break
one style on purpose, run the flow, and see the broken element named in the
stack output.

### What is known

- The diff PNG is in device pixels at DPR 2 (the only scale this project
  uses). For an element capture, diff pixel `(px, py)` maps to CSS viewport
  coordinates `(box.x + px / 2, box.y + py / 2)`, where `box` comes from
  `agent-browser get box <selector>`.
- `document.elementsFromPoint(x, y)`, run through `agent-browser eval --stdin`,
  returns the DOM stack, topmost first. It takes viewport coordinates, so the
  target must be scrolled into view and the viewport must match the capture.
- `agent-browser open --enable react-devtools` adds `react tree` and
  `react inspect <id>`. Whether a DOM node can be mapped to its component
  without that flag is not verified; check before relying on it.
- `kuk` has the per-pixel similarity map in memory (`compare::CompareResult`,
  rendered by `overlay.rs`). It reports one score today and no regions.
- These were checked against `agent-browser` 0.33.1. Re-check with
  `agent-browser <command> --help`.

### Decisions to settle with Adrian before any code

Ask them one at a time, with a recommendation.

1. The split between `kuk` and the skill. Recommended: `kuk` stays
   browser-free and learns to report regions; the browser query lives in the
   skill as a helper script next to `figma-fetch.sh`. The alternative is a
   `kuk` subcommand that shells out to `agent-browser`, which ties the binary
   to one browser tool.
2. How regions are reported: extra rows on the normal report, or a `regions`
   subcommand. `CLAUDE.md` says new capabilities are subcommands.
3. How red pixels become regions: the clustering method, the minimum size
   worth reporting, and what counts as "red" relative to `--gain`.
4. Which fields a stack row carries, and where TOON encoding happens if the
   query is a shell script (`kuk` has the `toon-format` crate; a script does
   not).
5. Whether React is the only component model in scope for the first version.

### Constraints

- Output for agents is small by default. Regions and stack rows are large
  uniform lists: project them to flat fields (for example depth, tag, test id,
  component, box) and emit TOON. Never hand the agent raw `react inspect`
  output or DOM JSON. Text output has a budget of about 2,000 tokens and says
  on stderr when it trimmed; `--json` is never trimmed, only `--limit` caps it.
- JSON and TOON payloads are a versioned contract (`report::SCHEMA_VERSION`).
  Add fields; do not rename or remove them.
- Exit codes `0`, `1`, `2`, `3` keep their meaning.
- The skill invariants in `CLAUDE.md` stand: Figma desktop MCP required,
  `--scale 2` only, no `sips -z`.
- A change to the CLI surface adds an integration test in `tests/cli.rs`, and
  `kuk schema` must describe it without hand-written additions.
- Never read or commit `/.design-browser/`; it holds session cookies.

### How to work

1. Spike the browser side first, on a real page, by hand: capture an element,
   pick a pixel, run `elementsFromPoint`, look at what comes back. The shape
   of that output decides most of the questions above. Keep the spike out of
   the repo.
2. Write the spec with Adrian (`docs/plans/`), then a plan with exact files
   and verification per task.
3. Implement test-first where the task lists tests. Run
   `cargo fmt --all -- --check`, `cargo build --release --locked`,
   `cargo test --release --locked` and
   `cargo clippy --all-targets -- -D warnings` before calling a task done.
4. Update `CHANGELOG.md` under `[Unreleased]`, `README.md`, and
   `skills/kuk-compare/SKILL.md` (the "Autonomous diff analysis" section is
   where the new flow replaces vision guessing).
5. Work in a gittr worktree. Commit freely; do not push or open a PR without
   being asked.

### Not in scope

- Changing the comparison algorithm or the score.
- Fixing the implementation's CSS automatically.
- Design-side semantics (mapping a point to a Figma node). Note it as a
  follow-up if the spike shows it is cheap.
