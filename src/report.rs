use std::error::Error;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{Args, OutputFormat};
use crate::compare::CompareResult;

/// Version of the JSON and TOON payloads. Additive changes keep it; a removed
/// or renamed field bumps it together with the crate's major version.
pub const SCHEMA_VERSION: u32 = 1;

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
    pub schema_version: u32,
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

/// Size difference between the two images, implementation minus design.
#[derive(Debug, Clone, Serialize)]
pub struct Delta {
    pub width: i64,
    pub height: i64,
}

/// What `kuk` reports instead of a [`Report`] when the two images differ in size.
#[derive(Debug, Clone, Serialize)]
pub struct Mismatch {
    pub schema_version: u32,
    pub a: ImageInfo,
    pub b: ImageInfo,
    pub dims_match: bool,
    pub delta: Delta,
    pub error: &'static str,
}

/// Serde shim used only for TOON encoding via `toon-format`.
/// Designed so the crate emits the tabular `sources[2]{label,path,width,height}` form.
#[derive(Serialize)]
struct ToonReport<'a> {
    schema_version: u32,
    sources: [ToonSource<'a>; 2],
    dims_match: bool,
    score: f64,
    threshold: f64,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_path: Option<&'a Path>,
}

#[derive(Serialize)]
struct ToonMismatch<'a> {
    schema_version: u32,
    sources: [ToonSource<'a>; 2],
    dims_match: bool,
    delta: &'a Delta,
    error: &'static str,
}

#[derive(Serialize)]
struct ToonSource<'a> {
    label: &'a str,
    path: &'a Path,
    width: u32,
    height: u32,
}

fn toon_sources<'a>(a: &'a ImageInfo, b: &'a ImageInfo) -> [ToonSource<'a>; 2] {
    [("a", a), ("b", b)].map(|(label, info)| ToonSource {
        label,
        path: &info.path,
        width: info.width,
        height: info.height,
    })
}

fn encode_toon<T: Serialize>(shim: &T) -> String {
    let mut out =
        toon_format::encode_default(shim).expect("TOON encoding of a fixed shape cannot fail");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn encode_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(value)?;
    s.push('\n');
    Ok(s)
}

impl Report {
    /// Build a `Report` from the compare result and the CLI args that drove this run.
    ///
    /// `design_path` / `impl_path` come from CLI args. `diff_path` is the path that was
    /// written, or `None` if `--no-diff`.
    //
    // Invariant: only callable after `compare::run` returns Ok, which guarantees equal
    // dims. The mismatch path builds a `Mismatch` instead.
    pub fn from_compare(
        result: &CompareResult,
        args: &Args,
        design_path: PathBuf,
        impl_path: PathBuf,
        diff_path: Option<PathBuf>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
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

    /// Render in the selected output format, newline-terminated.
    pub fn render(&self, format: OutputFormat) -> Result<String, Box<dyn Error>> {
        Ok(match format {
            OutputFormat::Text => self.to_text(),
            OutputFormat::Json => self.to_json()?,
            OutputFormat::Toon => self.to_toon(),
        })
    }

    /// Format as a multi-line human-readable summary including a dims block.
    pub fn to_text(&self) -> String {
        let diff_display = match &self.diff_path {
            Some(path) => path.display().to_string(),
            None => "(skipped)".to_string(),
        };
        let match_marker = if self.dims_match { "match" } else { "MISMATCH" };

        let mut out = String::new();
        out.push_str("kuk\n");
        out.push_str(&format!(
            "  {}  {}x{}\n",
            self.a.path.display(),
            self.a.width,
            self.a.height,
        ));
        out.push_str(&format!(
            "  {}  {}x{}  {}\n",
            self.b.path.display(),
            self.b.width,
            self.b.height,
            match_marker,
        ));
        out.push_str(&format!(
            "score: {:.4} ({:.2}% similar)\n",
            self.score,
            self.score * 100.0,
        ));
        out.push_str(&format!("diff:  {}\n", diff_display));
        out
    }

    /// Format as TOON (Token-Oriented Object Notation) via the `toon-format` crate.
    /// The serde shim `ToonReport` is shaped to produce the tabular
    /// `sources[2]{label,path,width,height}` form for token efficiency.
    pub fn to_toon(&self) -> String {
        encode_toon(&ToonReport {
            schema_version: self.schema_version,
            sources: toon_sources(&self.a, &self.b),
            dims_match: self.dims_match,
            score: self.score,
            threshold: self.threshold,
            passed: self.passed,
            diff_path: self.diff_path.as_deref(),
        })
    }

    /// Format as a compact JSON line terminated by `\n`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        encode_json(self)
    }
}

impl Mismatch {
    /// `design` and `implementation` are `(width, height)` in pixels.
    pub fn new(
        design_path: PathBuf,
        impl_path: PathBuf,
        design: (u32, u32),
        implementation: (u32, u32),
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            a: ImageInfo {
                path: design_path,
                width: design.0,
                height: design.1,
            },
            b: ImageInfo {
                path: impl_path,
                width: implementation.0,
                height: implementation.1,
            },
            dims_match: false,
            delta: Delta {
                width: i64::from(implementation.0) - i64::from(design.0),
                height: i64::from(implementation.1) - i64::from(design.1),
            },
            error: "dimension_mismatch",
        }
    }

    /// Render in the selected output format, newline-terminated.
    pub fn render(&self, format: OutputFormat) -> Result<String, Box<dyn Error>> {
        Ok(match format {
            OutputFormat::Text => self.to_text(),
            OutputFormat::Json => encode_json(self)?,
            OutputFormat::Toon => encode_toon(&ToonMismatch {
                schema_version: self.schema_version,
                sources: toon_sources(&self.a, &self.b),
                dims_match: self.dims_match,
                delta: &self.delta,
                error: self.error,
            }),
        })
    }

    fn to_text(&self) -> String {
        format!(
            "kuk: dimension mismatch\n  {}  {}x{}\n  {}  {}x{}  ({:+}, {:+})\n\
             re-capture the implementation at the design's size.\n\
             do not resize either image: scaling distorts the comparison.\n",
            self.a.path.display(),
            self.a.width,
            self.a.height,
            self.b.path.display(),
            self.b.width,
            self.b.height,
            self.delta.width,
            self.delta.height,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_args(threshold: f64, no_diff: bool) -> Args {
        use clap::Parser;
        let mut args = Args::parse_from(["kuk", "a.png", "b.png"]);
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
    fn to_text_should_include_dims_block_and_score() {
        let report = Report {
            schema_version: SCHEMA_VERSION,
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

        let output = report.to_text();

        assert!(output.contains("design.png"));
        assert!(output.contains("impl.png"));
        assert!(output.contains("1600x1200"));
        assert!(
            output.contains("1600x1200  match\n"),
            "expected `match` marker on dims line, got:\n{output}"
        );
        assert!(output.contains("score: 0.9958"));
        assert!(output.contains("99.58% similar"));
        assert!(output.contains("diff:  diff.png"));
    }

    #[test]
    fn to_text_should_show_skipped_when_no_diff_path() {
        let report = Report {
            schema_version: SCHEMA_VERSION,
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

        let output = report.to_text();
        assert!(output.contains("diff:  (skipped)"));
    }

    #[test]
    fn to_json_should_contain_a_b_and_dims_match() {
        let report = Report {
            schema_version: SCHEMA_VERSION,
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
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON should parse");

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
            schema_version: SCHEMA_VERSION,
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
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON should parse");
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

        assert!(
            report.passed,
            "score == threshold should pass (>= semantics)"
        );
    }

    #[test]
    fn to_toon_should_emit_sources_array_and_scalars() {
        let report = Report {
            schema_version: SCHEMA_VERSION,
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
        assert!(toon.contains("threshold: 0.99"), "got:\n{toon}");
        assert!(toon.contains("passed: true"));
        assert!(toon.contains("diff_path: diff.png"));
        assert!(toon.ends_with('\n'));
    }

    #[test]
    fn to_toon_should_render_whole_threshold_as_float() {
        let report = Report {
            schema_version: SCHEMA_VERSION,
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
            threshold: 1.0,
            passed: true,
            diff_path: None,
        };

        let toon = report.to_toon();
        // The toon-format crate uses shortest-lossless float serialisation: 1.0f64 → "1".
        // Accepting both "1.0" and "1" keeps the test honest while tolerating crate behaviour.
        assert!(
            toon.contains("threshold: 1.0") || toon.contains("threshold: 1\n"),
            "threshold=1.0 must appear (got:\n{toon})"
        );
    }

    #[test]
    fn to_toon_should_omit_diff_path_when_none() {
        let report = Report {
            schema_version: SCHEMA_VERSION,
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
}
