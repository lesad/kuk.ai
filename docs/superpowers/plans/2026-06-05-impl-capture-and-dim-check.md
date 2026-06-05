# Impl-capture flow + dim reporting + `--format` flag Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add format-aware output (`human`/`json`/`toon`), surface both inputs' dimensions in every result, route dimension mismatch through a structured stdout renderer with a distinct exit code (3), and rewrite the `peep-compare` skill's Step 2 to use Chrome DevTools "Capture node screenshot".

**Architecture:** Replace the `--json` boolean with a `--format` enum that selects one of three lowercase variants. Expand `Report` to carry both inputs' paths + dimensions and a `dims_match` flag. Route the existing `PeepError::DimMismatch` through a new format-aware renderer (stdout, exit 3) instead of the generic stderr+2 path. TOON encoding is hand-rolled for our fixed schema — no new dependency.

**Tech Stack:** Rust 2024 edition, `clap` (v4 derive), `serde` + `serde_json`, `thiserror`, `image-compare`. Tests use `assert_cmd`, `tempfile`, `predicates`. Verification follows TDD.

**Spec:** `docs/superpowers/specs/2026-06-05-impl-capture-and-dim-check.md`

---

## Task 1: Replace `--json` bool with `--format human|json|toon` enum

Pure refactor — no shape change yet. Establishes the new flag surface and keeps every test green by migrating call sites in lock-step.

**Files:**
- Modify: `src/cli.rs` (add `OutputFormat` enum, replace `json: bool` with `format: OutputFormat`, update unit tests)
- Modify: `src/main.rs:20,63-73,88-99` (update `print_report` signature + body, update `make_args` test helper)
- Modify: `tests/cli.rs` (replace `--json` with `--format json` in every integration test; update help-output assertion)

- [ ] **Step 1: Update the unit tests in `src/cli.rs` to expect the new `format` field**

Replace the existing `tests` module in `src/cli.rs` (lines 48–97) with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defaults_should_be_set_correctly() {
        let args = Args::try_parse_from(["peep", "a.png", "b.png"]).expect("parse should succeed");

        assert_eq!(args.design, PathBuf::from("a.png"));
        assert_eq!(args.implementation, PathBuf::from("b.png"));
        assert_eq!(args.output, PathBuf::from("diff.png"));
        assert!((args.threshold - 0.99).abs() < f64::EPSILON);
        assert!((args.gain - 4.0).abs() < f32::EPSILON);
        assert!(!args.fail);
        assert_eq!(args.format, OutputFormat::Human);
        assert!(!args.no_diff);
    }

    #[test]
    fn all_flags_overridden_should_reflect_new_values() {
        let args = Args::try_parse_from([
            "peep",
            "a.png",
            "b.png",
            "--output",
            "out.png",
            "--threshold",
            "0.95",
            "--gain",
            "8.0",
            "--fail",
            "--format",
            "json",
            "--no-diff",
        ])
        .expect("parse should succeed");

        assert_eq!(args.output, PathBuf::from("out.png"));
        assert!((args.threshold - 0.95).abs() < f64::EPSILON);
        assert!((args.gain - 8.0).abs() < f32::EPSILON);
        assert!(args.fail);
        assert_eq!(args.format, OutputFormat::Json);
        assert!(args.no_diff);
    }

    #[test]
    fn format_toon_should_parse() {
        let args = Args::try_parse_from(["peep", "a.png", "b.png", "--format", "toon"])
            .expect("parse should succeed");
        assert_eq!(args.format, OutputFormat::Toon);
    }

    #[test]
    fn format_uppercase_should_be_rejected() {
        let result = Args::try_parse_from(["peep", "a.png", "b.png", "--format", "JSON"]);
        assert!(result.is_err(), "uppercase format values must be rejected");
    }

    #[test]
    fn missing_implementation_argument_should_error() {
        let result = Args::try_parse_from(["peep", "a.png"]);
        assert!(result.is_err(), "expected parse error when IMPL is missing");
    }
}
```

- [ ] **Step 2: Run the unit tests; verify they fail to compile**

Run: `cargo test --lib cli::tests`
Expected: compile error — `OutputFormat` not in scope, `args.format` field does not exist.

- [ ] **Step 3: Add the `OutputFormat` enum and replace the `json` field in `src/cli.rs`**

Replace the contents of `src/cli.rs` (everything **before** the `#[cfg(test)] mod tests { ... }` block, lines 1–46) with:

```rust
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Output format selector. CLI accepts only lowercase variants.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[clap(rename_all = "lower")]
pub enum OutputFormat {
    /// Default. Multi-line human-readable report.
    Human,
    /// Compact single-line JSON.
    Json,
    /// TOON (Token-Oriented Object Notation) — compact, agent-friendly.
    Toon,
}

#[derive(Parser, Debug)]
#[command(
    name = "peep",
    version,
    about = "Compare two webpage screenshots and emit a similarity score + red-overlay diff PNG"
)]
pub struct Args {
    /// Path to the design (reference) image.
    pub design: PathBuf,

    /// Path to the implementation image.
    #[arg(value_name = "IMPL")]
    pub implementation: PathBuf,

    /// Where to write the diff PNG.
    #[arg(short, long, default_value = "diff.png", value_name = "PATH")]
    pub output: PathBuf,

    /// Minimum acceptable similarity (0.0 = totally dissimilar, 1.0 = identical).
    #[arg(short, long, default_value_t = 0.99, value_name = "F64")]
    pub threshold: f64,

    /// Visibility gain applied to per-pixel diffs when rendering the overlay.
    /// Higher = exaggerate smaller differences. Typical 4.0.
    #[arg(long, default_value_t = 4.0, value_name = "F32")]
    pub gain: f32,

    /// Exit with code 1 when score < threshold (useful in CI).
    #[arg(long)]
    pub fail: bool,

    /// Output format. `human` is multi-line text, `json` is compact JSON,
    /// `toon` is TOON (token-efficient for agent contexts).
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, value_name = "FORMAT")]
    pub format: OutputFormat,

    /// Skip writing the diff PNG.
    #[arg(long)]
    pub no_diff: bool,
}

pub(crate) fn parse() -> Args {
    Args::parse()
}
```

- [ ] **Step 4: Update `print_report` in `src/main.rs` to dispatch on `OutputFormat`**

Replace lines 63–73 of `src/main.rs` with:

```rust
fn print_report(report: &Report, format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Human => out.write_all(report.to_human().as_bytes())?,
        OutputFormat::Json => out.write_all(report.to_json()?.as_bytes())?,
        OutputFormat::Toon => out.write_all(report.to_toon().as_bytes())?,
    }
    Ok(())
}
```

Also update the call site at line 20:

```rust
            if let Err(e) = print_report(&report, args.format) {
```

And add the import at line 10 — change:

```rust
use crate::cli::Args;
```

to:

```rust
use crate::cli::{Args, OutputFormat};
```

**Note:** `report.to_toon()` is added in Task 3. Until then `cargo build` will fail with "method `to_toon` not found on `&Report`". That's expected — Task 1 ends with `cargo test` confirming the cli unit tests + main.rs unit tests parse correctly. Use `cargo test --lib cli::tests` and `cargo test --lib --no-run` to bound the surface for this task. Full `cargo test` becomes green at the end of Task 4.

Actually — to avoid leaving the build broken between tasks, swap the dispatch to **not** reference `to_toon` yet. Use this body for `print_report` instead, until Task 4:

```rust
fn print_report(report: &Report, format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Human => out.write_all(report.to_human().as_bytes())?,
        OutputFormat::Json => out.write_all(report.to_json()?.as_bytes())?,
        OutputFormat::Toon => {
            // Wired up in Task 4. Falls back to JSON for now so the build stays green.
            out.write_all(report.to_json()?.as_bytes())?;
        }
    }
    Ok(())
}
```

Replace this body in Task 4 with the real `to_toon` call.

- [ ] **Step 5: Update `make_args` test helper in `src/main.rs`**

Replace the helper (lines 88–99 of `src/main.rs`) with:

```rust
    fn make_args(design: PathBuf, implementation: PathBuf, output: PathBuf) -> Args {
        Args {
            design,
            implementation,
            output,
            threshold: 0.99,
            gain: 4.0,
            fail: false,
            format: OutputFormat::Human,
            no_diff: false,
        }
    }
```

Add to the test-module imports (just below `use super::*;` at line 77):

```rust
    use crate::cli::OutputFormat;
```

- [ ] **Step 6: Migrate the integration tests in `tests/cli.rs` from `--json` to `--format json`**

In `tests/cli.rs`, replace every occurrence of `.arg("--json")` with `.arg("--format").arg("json")`. That's lines 83, 132, 213.

Also update the help test (lines 299–311) to assert `--format` instead of `--json`. Replace that test with:

```rust
#[test]
fn help_flag_should_exit_0_and_show_all_flags() {
    peep()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("--output"))
        .stdout(contains("--threshold"))
        .stdout(contains("--gain"))
        .stdout(contains("--fail"))
        .stdout(contains("--format"))
        .stdout(contains("human"))
        .stdout(contains("json"))
        .stdout(contains("toon"))
        .stdout(contains("--no-diff"))
        .stdout(contains("IMPL"));
}
```

- [ ] **Step 7: Run the full test suite; verify it passes**

Run: `cargo test`
Expected: all tests pass. No `--json` references remain in production code or tests.

- [ ] **Step 8: Commit**

```bash
git add src/cli.rs src/main.rs tests/cli.rs
git commit -m "feat(cli): replace --json with --format human|json|toon"
```

---

## Task 2: Expand `Report` with `a`/`b`/`dims_match` and update `to_human` + `to_json`

Carry both inputs' paths + dimensions through the report. Drop bare `width`/`height`. Echo dims block in human output. Restructure JSON shape.

**Files:**
- Modify: `src/report.rs` (struct, `from_compare` signature, `to_human`, `to_json`, unit tests)
- Modify: `src/main.rs::run` (pass `design` and `implementation` paths into `Report::from_compare`)
- Modify: `tests/cli.rs` (existing `--format json` test asserts new shape; add `--format human` test for dims block)

- [ ] **Step 1: Write the failing unit tests in `src/report.rs`**

Replace the entire `#[cfg(test)] mod tests { ... }` block at the bottom of `src/report.rs` (lines 61–191) with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_args(threshold: f64, no_diff: bool) -> Args {
        use clap::Parser;
        let mut args = Args::parse_from(["peep", "a.png", "b.png"]);
        args.threshold = threshold;
        args.no_diff = no_diff;
        args
    }

    #[test]
    fn from_compare_should_populate_all_fields_correctly() {
        let result = CompareResult::test_fixture(0.9758, 800, 600);
        let args = make_args(0.95, false);
        let diff_path = Some(PathBuf::from("out.png"));

        let report = Report::from_compare(
            &result,
            &args,
            PathBuf::from("design.png"),
            PathBuf::from("impl.png"),
            diff_path.clone(),
        );

        assert!((report.score - 0.9758).abs() < f64::EPSILON);
        assert!((report.threshold - 0.95).abs() < f64::EPSILON);
        assert!(report.passed);
        assert_eq!(report.a.path, PathBuf::from("design.png"));
        assert_eq!(report.a.width, 800);
        assert_eq!(report.a.height, 600);
        assert_eq!(report.b.path, PathBuf::from("impl.png"));
        assert_eq!(report.b.width, 800);
        assert_eq!(report.b.height, 600);
        assert!(report.dims_match);
        assert_eq!(report.diff_path, diff_path);
    }

    #[test]
    fn to_human_should_include_dims_block_and_score() {
        let report = Report {
            a: ImageInfo {
                path: PathBuf::from("design.png"),
                width: 1600,
                height: 1200,
            },
            b: ImageInfo {
                path: PathBuf::from("impl.png"),
                width: 1600,
                height: 1200,
            },
            dims_match: true,
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            diff_path: Some(PathBuf::from("diff.png")),
        };

        let output = report.to_human();

        assert!(output.contains("design.png"));
        assert!(output.contains("impl.png"));
        assert!(output.contains("1600x1200"));
        assert!(output.contains("match"));
        assert!(output.contains("score: 0.9958"));
        assert!(output.contains("99.58% similar"));
        assert!(output.contains("diff:  diff.png"));
    }

    #[test]
    fn to_human_should_show_skipped_when_no_diff_path() {
        let report = Report {
            a: ImageInfo {
                path: PathBuf::from("a.png"),
                width: 100,
                height: 200,
            },
            b: ImageInfo {
                path: PathBuf::from("b.png"),
                width: 100,
                height: 200,
            },
            dims_match: true,
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            diff_path: None,
        };

        let output = report.to_human();
        assert!(output.contains("diff:  (skipped)"));
    }

    #[test]
    fn to_json_should_contain_a_b_and_dims_match() {
        let report = Report {
            a: ImageInfo {
                path: PathBuf::from("design.png"),
                width: 1600,
                height: 1200,
            },
            b: ImageInfo {
                path: PathBuf::from("impl.png"),
                width: 1600,
                height: 1200,
            },
            dims_match: true,
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            diff_path: Some(PathBuf::from("diff.png")),
        };

        let json = report.to_json().expect("to_json must succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should parse");

        assert_eq!(parsed["a"]["path"].as_str(), Some("design.png"));
        assert_eq!(parsed["a"]["width"].as_u64(), Some(1600));
        assert_eq!(parsed["a"]["height"].as_u64(), Some(1200));
        assert_eq!(parsed["b"]["path"].as_str(), Some("impl.png"));
        assert_eq!(parsed["b"]["width"].as_u64(), Some(1600));
        assert_eq!(parsed["b"]["height"].as_u64(), Some(1200));
        assert_eq!(parsed["dims_match"].as_bool(), Some(true));
        assert!((parsed["score"].as_f64().unwrap() - 0.9958).abs() < 1e-10);
        assert_eq!(parsed["passed"].as_bool(), Some(true));
        assert_eq!(parsed["diff_path"].as_str(), Some("diff.png"));
        assert!(json.ends_with('\n'));
        assert!(!json.trim_end_matches('\n').contains('\n'));
    }

    #[test]
    fn to_json_should_omit_diff_path_when_none() {
        let report = Report {
            a: ImageInfo {
                path: PathBuf::from("a.png"),
                width: 10,
                height: 10,
            },
            b: ImageInfo {
                path: PathBuf::from("b.png"),
                width: 10,
                height: 10,
            },
            dims_match: true,
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            diff_path: None,
        };

        let json = report.to_json().expect("to_json must succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should parse");
        assert!(parsed.get("diff_path").is_none());
    }

    #[test]
    fn from_compare_should_set_passed_false_when_score_below_threshold() {
        let result = CompareResult::test_fixture(0.5, 10, 10);
        let args = make_args(0.99, false);

        let report = Report::from_compare(
            &result,
            &args,
            PathBuf::from("a.png"),
            PathBuf::from("b.png"),
            None,
        );

        assert!(!report.passed);
    }

    #[test]
    fn from_compare_passed_when_score_equals_threshold() {
        let result = CompareResult::test_fixture(0.99, 10, 10);
        let args = make_args(0.99, false);

        let report = Report::from_compare(
            &result,
            &args,
            PathBuf::from("a.png"),
            PathBuf::from("b.png"),
            None,
        );

        assert!(report.passed, "score == threshold should pass (>= semantics)");
    }
}
```

- [ ] **Step 2: Run the unit tests; verify they fail to compile**

Run: `cargo test --lib report::tests`
Expected: compile error — `ImageInfo` does not exist, `Report` fields `a`/`b`/`dims_match` do not exist, `from_compare` signature mismatch.

- [ ] **Step 3: Replace the production code in `src/report.rs`**

Replace lines 1–59 of `src/report.rs` (the entire pre-test section) with:

```rust
use std::path::PathBuf;

use serde::Serialize;

use crate::cli::Args;
use crate::compare::CompareResult;

/// Path + dimensions for one side of a comparison.
#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// Outcome of a single comparison, suitable for both human and machine rendering.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub a: ImageInfo,
    pub b: ImageInfo,
    pub dims_match: bool,
    pub score: f64,
    pub threshold: f64,
    pub passed: bool,
    /// Where the diff PNG was written, or `None` if `--no-diff`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_path: Option<PathBuf>,
}

impl Report {
    /// Build a `Report` from the compare result and the CLI args that drove this run.
    ///
    /// `design_path` / `impl_path` come from CLI args. `diff_path` is the path that was
    /// written, or `None` if `--no-diff`.
    pub fn from_compare(
        result: &CompareResult,
        args: &Args,
        design_path: PathBuf,
        impl_path: PathBuf,
        diff_path: Option<PathBuf>,
    ) -> Self {
        Self {
            a: ImageInfo {
                path: design_path,
                width: result.width,
                height: result.height,
            },
            b: ImageInfo {
                path: impl_path,
                width: result.width,
                height: result.height,
            },
            dims_match: true,
            score: result.score,
            threshold: args.threshold,
            passed: result.score >= args.threshold,
            diff_path,
        }
    }

    /// Format as a multi-line human-readable summary including a dims block.
    pub fn to_human(&self) -> String {
        let diff_display = match &self.diff_path {
            Some(path) => path.display().to_string(),
            None => "(skipped)".to_string(),
        };
        let match_marker = if self.dims_match { "match" } else { "MISMATCH" };
        format!(
            "peep\n  {a_path}  {a_w}x{a_h}\n  {b_path}  {b_w}x{b_h}  {match_marker}\nscore: {score:.4} ({pct:.2}% similar)\ndiff:  {diff_display}\n",
            a_path = self.a.path.display(),
            a_w = self.a.width,
            a_h = self.a.height,
            b_path = self.b.path.display(),
            b_w = self.b.width,
            b_h = self.b.height,
            match_marker = match_marker,
            score = self.score,
            pct = self.score * 100.0,
            diff_display = diff_display,
        )
    }

    /// Format as a compact JSON line terminated by `\n`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}
```

- [ ] **Step 4: Update `src/main.rs::run` to thread paths into `Report::from_compare`**

Replace lines 43–61 of `src/main.rs` with:

```rust
fn run(args: &Args) -> Result<Report, PeepError> {
    let result = compare::run(&args.design, &args.implementation)?;
    let diff_path = if args.no_diff {
        None
    } else {
        let diff = overlay::render(
            &result.impl_image,
            &result.similarity,
            OVERLAY_COLOR,
            args.gain,
        );
        diff.save(&args.output).map_err(|e| PeepError::DiffWrite {
            path: args.output.clone(),
            source: e,
        })?;
        Some(args.output.clone())
    };
    Ok(Report::from_compare(
        &result,
        args,
        args.design.clone(),
        args.implementation.clone(),
        diff_path,
    ))
}
```

- [ ] **Step 5: Update integration tests in `tests/cli.rs` for the new JSON shape**

Replace the body of `identical_pngs_with_json_flag_should_produce_valid_json_with_passed_true` (lines 68–106). Rename it to match the new flag:

```rust
#[test]
fn identical_pngs_with_format_json_should_produce_valid_json_with_a_b_blocks() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 32, 32, Rgba([255, 0, 0, 255]));
    write_solid_png(&b, 32, 32, Rgba([255, 0, 0, 255]));

    let output = peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("stdout should be valid JSON");

    assert_eq!(json["dims_match"].as_bool(), Some(true));
    assert_eq!(json["a"]["width"].as_u64(), Some(32));
    assert_eq!(json["a"]["height"].as_u64(), Some(32));
    assert_eq!(json["b"]["width"].as_u64(), Some(32));
    assert_eq!(json["b"]["height"].as_u64(), Some(32));
    assert!(
        json["a"]["path"]
            .as_str()
            .expect("a.path should be string")
            .ends_with("a.png")
    );
    assert!(
        json["b"]["path"]
            .as_str()
            .expect("b.path should be string")
            .ends_with("b.png")
    );
    assert_eq!(json["passed"].as_bool(), Some(true));
    assert!(json["score"].as_f64().unwrap_or(0.0) >= 0.999);
    let diff_path = json["diff_path"]
        .as_str()
        .expect("diff_path should be a string");
    assert!(!diff_path.is_empty(), "diff_path should be non-empty");
}
```

Also, the `different_pngs_without_fail_flag_should_exit_0_with_score_below_threshold` test (lines 109–154) keeps its body but its existing JSON parsing still works because `score` and `passed` are still top-level fields. Same for `different_pngs_with_fail_and_json_should_exit_1_with_passed_false` (lines 188–229). No change required there beyond the `--format json` arg substitution already done in Task 1.

- [ ] **Step 6: Add a `--format human` integration test asserting the dims block**

Append this test to `tests/cli.rs`:

```rust
#[test]
fn identical_pngs_with_format_human_should_print_dims_block() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("design.png");
    let b = dir.path().join("impl.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 32, 32, Rgba([0, 0, 255, 255]));
    write_solid_png(&b, 32, 32, Rgba([0, 0, 255, 255]));

    peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .assert()
        .success()
        .stdout(contains("design.png"))
        .stdout(contains("impl.png"))
        .stdout(contains("32x32"))
        .stdout(contains("match"))
        .stdout(contains("score: 1.0000"));
}
```

- [ ] **Step 7: Run the full test suite; verify all green**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/report.rs src/main.rs tests/cli.rs
git commit -m "feat(report): expose both inputs' paths and dims; add dims_match flag"
```

---

## Task 3: Add hand-rolled `to_toon` method on `Report`

Add the third renderer. No CLI wiring yet — that's Task 4.

**Files:**
- Modify: `src/report.rs` (add `to_toon` method, add unit test)

- [ ] **Step 1: Write the failing unit test in `src/report.rs`**

Append to the `#[cfg(test)] mod tests` block in `src/report.rs`:

```rust
    #[test]
    fn to_toon_should_emit_sources_array_and_scalars() {
        let report = Report {
            a: ImageInfo {
                path: PathBuf::from("design.png"),
                width: 1600,
                height: 1200,
            },
            b: ImageInfo {
                path: PathBuf::from("impl.png"),
                width: 1600,
                height: 1200,
            },
            dims_match: true,
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            diff_path: Some(PathBuf::from("diff.png")),
        };

        let toon = report.to_toon();

        assert!(toon.contains("sources[2]{label,path,width,height}:"));
        assert!(toon.contains("a,design.png,1600,1200"));
        assert!(toon.contains("b,impl.png,1600,1200"));
        assert!(toon.contains("dims_match: true"));
        assert!(toon.contains("score: 0.9958"));
        assert!(toon.contains("threshold: 0.99"));
        assert!(toon.contains("passed: true"));
        assert!(toon.contains("diff_path: diff.png"));
        assert!(toon.ends_with('\n'));
    }

    #[test]
    fn to_toon_should_omit_diff_path_when_none() {
        let report = Report {
            a: ImageInfo {
                path: PathBuf::from("a.png"),
                width: 10,
                height: 10,
            },
            b: ImageInfo {
                path: PathBuf::from("b.png"),
                width: 10,
                height: 10,
            },
            dims_match: true,
            score: 1.0,
            threshold: 0.99,
            passed: true,
            diff_path: None,
        };

        let toon = report.to_toon();
        assert!(!toon.contains("diff_path:"));
    }
```

- [ ] **Step 2: Run the unit tests; verify they fail**

Run: `cargo test --lib report::tests::to_toon`
Expected: compile error — method `to_toon` not found on `Report`.

- [ ] **Step 3: Implement `to_toon` in `src/report.rs`**

Insert this method into the `impl Report` block (between `to_human` and `to_json`):

```rust
    /// Format as TOON (Token-Oriented Object Notation).
    /// Hand-rolled for our fixed schema. Two-space indent, comma delimiter,
    /// `[N]` count matches row count. Paths and integers don't need escaping.
    pub fn to_toon(&self) -> String {
        let mut out = String::new();
        out.push_str("sources[2]{label,path,width,height}:\n");
        out.push_str(&format!(
            "  a,{},{},{}\n",
            self.a.path.display(),
            self.a.width,
            self.a.height,
        ));
        out.push_str(&format!(
            "  b,{},{},{}\n",
            self.b.path.display(),
            self.b.width,
            self.b.height,
        ));
        out.push_str(&format!("dims_match: {}\n", self.dims_match));
        out.push_str(&format!("score: {:.4}\n", self.score));
        out.push_str(&format!("threshold: {}\n", self.threshold));
        out.push_str(&format!("passed: {}\n", self.passed));
        if let Some(path) = &self.diff_path {
            out.push_str(&format!("diff_path: {}\n", path.display()));
        }
        out
    }
```

- [ ] **Step 4: Run the unit tests; verify they pass**

Run: `cargo test --lib report::tests`
Expected: all `report::tests::*` tests pass, including both new `to_toon` tests.

- [ ] **Step 5: Commit**

```bash
git add src/report.rs
git commit -m "feat(report): add TOON renderer for compact agent output"
```

---

## Task 4: Wire `--format toon` end-to-end and add an integration test

Connect `print_report`'s `Toon` arm to the real renderer, and verify via CLI.

**Files:**
- Modify: `src/main.rs::print_report` (replace the temporary JSON fallback in the `Toon` arm with the real call)
- Modify: `tests/cli.rs` (add `--format toon` integration test)

- [ ] **Step 1: Add the failing integration test**

Append to `tests/cli.rs`:

```rust
#[test]
fn identical_pngs_with_format_toon_should_contain_sources_header_and_match() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("design.png");
    let b = dir.path().join("impl.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 32, 32, Rgba([10, 20, 30, 255]));
    write_solid_png(&b, 32, 32, Rgba([10, 20, 30, 255]));

    let output = peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--format")
        .arg("toon")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let toon = String::from_utf8(output).expect("stdout should be utf-8");

    assert!(
        toon.contains("sources[2]{label,path,width,height}:"),
        "expected TOON header, got:\n{toon}"
    );
    assert!(toon.contains("dims_match: true"), "got:\n{toon}");
    assert!(toon.contains("passed: true"), "got:\n{toon}");
    assert!(
        toon.lines().any(|l| l.contains("a,") && l.contains("32,32")),
        "expected `a` row with 32x32, got:\n{toon}"
    );
    assert!(
        toon.lines().any(|l| l.contains("b,") && l.contains("32,32")),
        "expected `b` row with 32x32, got:\n{toon}"
    );
}
```

- [ ] **Step 2: Run the test; verify it fails**

Run: `cargo test --test cli identical_pngs_with_format_toon_should_contain_sources_header_and_match`
Expected: FAIL — the `Toon` arm currently falls back to JSON, so `sources[2]{...}` won't appear in stdout.

- [ ] **Step 3: Replace the temporary `Toon` arm in `src/main.rs::print_report`**

In `src/main.rs::print_report`, replace the `Toon` arm with the real call:

```rust
        OutputFormat::Toon => out.write_all(report.to_toon().as_bytes())?,
```

The final body looks like:

```rust
fn print_report(report: &Report, format: OutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Human => out.write_all(report.to_human().as_bytes())?,
        OutputFormat::Json => out.write_all(report.to_json()?.as_bytes())?,
        OutputFormat::Toon => out.write_all(report.to_toon().as_bytes())?,
    }
    Ok(())
}
```

- [ ] **Step 4: Run all tests; verify green**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs tests/cli.rs
git commit -m "feat(cli): wire --format toon end-to-end"
```

---

## Task 5: Route `PeepError::DimMismatch` to structured stdout + exit 3

Currently mismatch is handled by the generic error arm (stderr + exit 2). Move it to a format-aware stdout renderer with a distinct exit code.

**Files:**
- Modify: `src/main.rs` (add `print_mismatch`, branch in `main()`, exit 3)
- Modify: `tests/cli.rs` (rewrite existing mismatch test for exit 3 + stdout; add JSON and TOON variants)

- [ ] **Step 1: Rewrite the existing dimension-mismatch test in `tests/cli.rs`**

Replace `dimension_mismatch_should_exit_2_with_sizes_in_stderr` (lines 276–296) with:

```rust
#[test]
fn dimension_mismatch_human_should_exit_3_with_dims_on_stdout() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("small.png");
    let b = dir.path().join("large.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 4, 4, Rgba([255, 0, 0, 255]));
    write_solid_png(&b, 8, 8, Rgba([0, 255, 0, 255]));

    peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .assert()
        .failure()
        .code(3)
        .stdout(contains("dimension mismatch"))
        .stdout(contains("4x4"))
        .stdout(contains("8x8"));

    assert!(
        !out.exists(),
        "diff PNG must not be written on dimension mismatch"
    );
}
```

- [ ] **Step 2: Add the JSON mismatch test**

Append to `tests/cli.rs`:

```rust
#[test]
fn dimension_mismatch_format_json_should_exit_3_with_structured_payload() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("design.png");
    let b = dir.path().join("impl.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 4, 4, Rgba([255, 0, 0, 255]));
    write_solid_png(&b, 8, 8, Rgba([0, 255, 0, 255]));

    let output = peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--format")
        .arg("json")
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("stdout should be valid JSON on mismatch");

    assert_eq!(json["dims_match"].as_bool(), Some(false));
    assert_eq!(json["error"].as_str(), Some("dimension_mismatch"));
    assert_eq!(json["a"]["width"].as_u64(), Some(4));
    assert_eq!(json["a"]["height"].as_u64(), Some(4));
    assert_eq!(json["b"]["width"].as_u64(), Some(8));
    assert_eq!(json["b"]["height"].as_u64(), Some(8));
    assert_eq!(json["delta"]["width"].as_i64(), Some(4));
    assert_eq!(json["delta"]["height"].as_i64(), Some(4));
    assert!(!out.exists(), "diff PNG must not be written on mismatch");
}
```

- [ ] **Step 3: Add the TOON mismatch test**

Append to `tests/cli.rs`:

```rust
#[test]
fn dimension_mismatch_format_toon_should_exit_3_with_structured_payload() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("design.png");
    let b = dir.path().join("impl.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 4, 4, Rgba([255, 0, 0, 255]));
    write_solid_png(&b, 8, 8, Rgba([0, 255, 0, 255]));

    let output = peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--format")
        .arg("toon")
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();

    let toon = String::from_utf8(output).expect("stdout should be utf-8");

    assert!(
        toon.contains("sources[2]{label,path,width,height}:"),
        "got:\n{toon}"
    );
    assert!(toon.contains("dims_match: false"), "got:\n{toon}");
    assert!(toon.contains("error: dimension_mismatch"), "got:\n{toon}");
    assert!(toon.contains("delta:"), "got:\n{toon}");
    assert!(!out.exists());
}
```

- [ ] **Step 4: Run the three mismatch tests; verify they fail**

Run: `cargo test --test cli dimension_mismatch`
Expected: all three FAIL. The human one fails on exit code (currently 2) and stream (currently stderr). The JSON/TOON ones fail because mismatch currently emits free-text stderr, not structured stdout.

- [ ] **Step 5: Add `print_mismatch` to `src/main.rs`**

Insert this helper function in `src/main.rs` (above `main()`, after the constants):

```rust
fn print_mismatch(
    format: OutputFormat,
    design_path: &std::path::Path,
    impl_path: &std::path::Path,
    a: (u32, u32),
    b: (u32, u32),
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let (aw, ah) = a;
    let (bw, bh) = b;
    let dw = bw as i64 - aw as i64;
    let dh = bh as i64 - ah as i64;
    match format {
        OutputFormat::Human => {
            writeln!(out, "peep: dimension mismatch")?;
            writeln!(out, "  {}  {}x{}", design_path.display(), aw, ah)?;
            writeln!(
                out,
                "  {}  {}x{}  ({:+}, {:+})",
                impl_path.display(),
                bw,
                bh,
                dw,
                dh
            )?;
            writeln!(
                out,
                "resize externally (e.g. sips -z {ah} {aw} {impl} --out {impl})",
                impl = impl_path.display()
            )?;
            writeln!(out, "or re-capture if delta exceeds ~5%.")?;
        }
        OutputFormat::Json => {
            let payload = serde_json::json!({
                "a": { "path": design_path.display().to_string(), "width": aw, "height": ah },
                "b": { "path": impl_path.display().to_string(), "width": bw, "height": bh },
                "dims_match": false,
                "delta": { "width": dw, "height": dh },
                "error": "dimension_mismatch",
            });
            let mut s = serde_json::to_string(&payload)?;
            s.push('\n');
            out.write_all(s.as_bytes())?;
        }
        OutputFormat::Toon => {
            writeln!(out, "sources[2]{{label,path,width,height}}:")?;
            writeln!(out, "  a,{},{},{}", design_path.display(), aw, ah)?;
            writeln!(out, "  b,{},{},{}", impl_path.display(), bw, bh)?;
            writeln!(out, "dims_match: false")?;
            writeln!(out, "delta:")?;
            writeln!(out, "  width: {}", dw)?;
            writeln!(out, "  height: {}", dh)?;
            writeln!(out, "error: dimension_mismatch")?;
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Wire the `DimMismatch` arm in `main()`**

Replace the `Err(e)` arm of `main()` (currently lines 30–39 of `src/main.rs`) with:

```rust
        Err(PeepError::DimMismatch {
            width_a,
            height_a,
            width_b,
            height_b,
        }) => {
            if let Err(e) = print_mismatch(
                args.format,
                &args.design,
                &args.implementation,
                (width_a, height_a),
                (width_b, height_b),
            ) {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
            ExitCode::from(3)
        }
        Err(e) => {
            eprintln!("error: {e}");
            // Walk the error source chain for context, if any.
            let mut source = std::error::Error::source(&e);
            while let Some(s) = source {
                eprintln!("  caused by: {s}");
                source = s.source();
            }
            ExitCode::from(2)
        }
```

- [ ] **Step 7: Run all tests; verify green**

Run: `cargo test`
Expected: every test passes, including the three new mismatch tests.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs tests/cli.rs
git commit -m "feat(main): route dim mismatch through structured stdout + exit 3"
```

---

## Task 6: Bump crate version to 0.2.0

- [ ] **Step 1: Update `Cargo.toml`**

In `Cargo.toml`, change line 3 from:

```toml
version = "0.1.0"
```

to:

```toml
version = "0.2.0"
```

- [ ] **Step 2: Refresh `Cargo.lock` via a build**

Run: `cargo build`
Expected: builds cleanly. `Cargo.lock` reflects the new version.

- [ ] **Step 3: Run the test suite to confirm `--version` reports 0.2.0**

Run: `cargo test --test cli version_flag_should_exit_0_and_print_crate_version`
Expected: PASS. (The test uses `env!("CARGO_PKG_VERSION")`, so it adapts to the bump automatically.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.2.0"
```

---

## Task 7: Update `CHANGELOG.md`

- [ ] **Step 1: Replace the `[Unreleased]` section in `CHANGELOG.md`**

In `CHANGELOG.md`, replace lines 8–16 (the `[Unreleased]` section as it stands today) with:

```markdown
## [Unreleased]

### Added

- `tools/figma-fetch.sh` — bash helper that fetches a Figma node as PNG (or JPG/SVG/PDF) via the REST `/v1/images` endpoint. Writes to a self-describing default path under `$TMPDIR` and prints the path on stdout for shell capture; supports `--scale`, `--format`, `--absolute/--no-absolute`, and `--out PATH|-`. Requires `FIGMA_TOKEN` with `file_content:read` scope plus `curl` and `jq`.
- `--format <human|json|toon>` flag on `peep` selecting output format. Default `human`.
- Both inputs' dimensions are echoed on every successful run (in whichever format was selected).
- TOON output via `--format toon` for token-efficient agent consumption.
- Exit code `3` for dimension mismatch, distinct from generic error code `2`. Recoverable — callers can branch on it.

### Changed

- `peep-compare` skill: Step 1 rewritten to use hybrid Figma desktop MCP navigation plus REST fetch via `tools/figma-fetch.sh`. Documented that MCP image content blocks are vision-only and cannot be proxied to peep.
- `peep-compare` skill: Step 2 rewritten to use Chrome (or any Chromium-family) DevTools "Capture node screenshot" as the sole impl capture mechanism. The latest `~/Downloads/*.png` is copied to `/tmp/impl.png`. No new bundled tools.
- Dimension-mismatch report now goes to **stdout** in the selected format (was free-text on stderr in `0.1.0`). Same stream as the success report so callers parse once.

### Removed

- `--json` flag on `peep`. Replaced by `--format json`. No alias.
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): record 0.2.0 changes"
```

---

## Task 8: Update `README.md`

- [ ] **Step 1: Update the Status, Usage, and Tools sections of `README.md`**

In `README.md`, replace the **Status** section body (the paragraph under `## Status`) with:

```markdown
v0.2.0 — output format selector (`--format human|json|toon`), dimension reporting on every run, distinct exit code on dimension mismatch. Deferred for later: TOML config, multiple algorithms, side-by-side output, anti-aliasing toggle.
```

Replace the **Usage** section (the `## Usage` block, including the example output and flags list) with:

```markdown
## Usage

```sh
peep design.png impl.png
# peep
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
```

Leave the **Tools** section unchanged — it documents `tools/figma-fetch.sh` and is owned by the design-side work stream.

- [ ] **Step 2: Run the help test to verify the README example aligns with `--help` output**

Run: `cargo run -- --help`
Expected: `--format`, `human`, `json`, `toon`, and all other documented flags appear. Compare to the README example block.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): document --format flag and exit code 3"
```

---

## Task 9: Rewrite Step 2 of the `peep-compare` skill

This file lives outside the repo at `~/.claude/skills/peep-compare/SKILL.md`. If it's tracked in the user's dotfiles repo, commit there separately; otherwise the edit stands on its own filesystem.

**Files:**
- Modify: `~/.claude/skills/peep-compare/SKILL.md` (Required tools list + Step 2 body)

- [ ] **Step 1: Update the "Required tools" list**

Open `~/.claude/skills/peep-compare/SKILL.md`. In the "Required tools:" bullet list (around lines 12–18):

- **Remove** the `pngpaste` bullet entirely.
- **Add**, immediately after the `peep` bullet:

  ```markdown
  - Chrome (or any Chromium-family browser: Edge, Arc, Brave, Vivaldi) with DevTools (built-in) — used for impl capture in Step 2
  - `sips` — built-in on macOS; used only on dimension mismatch to resize externally
  ```

- [ ] **Step 2: Replace the body of Step 2**

In `~/.claude/skills/peep-compare/SKILL.md`, replace the entire "Step 2 — Capture the implementation" section (currently lines 43–49) with:

```markdown
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
```

- [ ] **Step 3: Run peep against a real Figma frame + real DevTools capture to smoke-test the new skill text**

Manual verification:

1. Fetch a known design: `DESIGN=$(tools/figma-fetch.sh <real-key> <real-node>)`
2. Open the matching implementation in Chrome at 100% zoom.
3. DevTools → Elements → right-click node → **Capture node screenshot**.
4. `IMPL=$(ls -t ~/Downloads/*.png | head -1); cp "$IMPL" /tmp/impl.png`
5. `peep "$DESIGN" /tmp/impl.png --format toon`

Expected: TOON output. If dims match, score appears. If not, structured mismatch report with exit 3 — at which point either `sips -z` and rerun, or re-capture.

- [ ] **Step 4: Commit the skill edit (only if the skills directory is git-tracked)**

If `~/.claude/skills/` is part of a git repo:

```bash
cd ~/.claude/skills
git add peep-compare/SKILL.md
git commit -m "docs(peep-compare): rewrite Step 2 for Chrome DevTools node-screenshot flow"
```

Otherwise the edit is local-only and persists via filesystem; no commit step.

---

## Self-review checklist (done before publishing this plan)

- **Spec coverage:** Each spec section maps to a task — CLI surface → Task 1; output shapes → Tasks 2–4; error model + exit codes → Task 5; version bump → Task 6; CHANGELOG → Task 7; README → Task 8; skill rewrite → Task 9. No spec section is uncovered.
- **Placeholder scan:** No "TBD", "TODO", "implement later", or "similar to Task N". Every code block contains the actual code to write.
- **Type consistency:** `OutputFormat` variants `Human/Json/Toon` are used identically across Tasks 1, 3, 4, 5. `ImageInfo { path, width, height }` is introduced in Task 2 and used unchanged in tests in Tasks 2–5. `Report::from_compare` signature `(result, args, design_path, impl_path, diff_path)` is consistent between Task 2's implementation and the test invocations.
- **Build greenness between tasks:** Task 1 leaves a temporary JSON fallback in the `Toon` arm of `print_report`; Task 4 swaps it for the real renderer. This is the only task boundary where the binary's behavior is briefly stubbed — tests at the boundary do not depend on the stubbed behavior.
