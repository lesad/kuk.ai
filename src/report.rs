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
    //
    // Invariant: only callable after `compare::run` returns Ok, which guarantees equal
    // dims. The mismatch path constructs Report (or its equivalent payload) directly.
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

        let mut out = String::new();
        out.push_str("peep\n");
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
        out.push_str(&format!("threshold: {:.4}\n", self.threshold));
        out.push_str(&format!("passed: {}\n", self.passed));
        if let Some(path) = &self.diff_path {
            out.push_str(&format!("diff_path: {}\n", path.display()));
        }
        out
    }

    /// Format as a compact JSON line terminated by `\n`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}

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
        assert!(
            output.contains("1600x1200  match\n"),
            "expected `match` marker on dims line, got:\n{output}"
        );
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
        assert!(toon.contains("threshold: 0.9900"));
        assert!(toon.contains("passed: true"));
        assert!(toon.contains("diff_path: diff.png"));
        assert!(toon.ends_with('\n'));
    }

    #[test]
    fn to_toon_should_render_whole_threshold_with_decimals() {
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
            threshold: 1.0,
            passed: true,
            diff_path: None,
        };

        let toon = report.to_toon();
        assert!(
            toon.contains("threshold: 1.0000"),
            "threshold=1.0 must serialize with decimals, got:\n{toon}"
        );
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
}
