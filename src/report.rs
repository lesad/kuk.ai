use std::path::PathBuf;

use serde::Serialize;

use crate::cli::Args;
use crate::compare::CompareResult;

/// Outcome of a single comparison, suitable for both human and machine rendering.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub score: f64,
    pub threshold: f64,
    pub passed: bool,
    pub width: u32,
    pub height: u32,
    /// Where the diff PNG was written, or `None` if `--no-diff`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_path: Option<PathBuf>,
}

#[expect(dead_code, reason = "wired in T4")]
impl Report {
    /// Build a `Report` from the compare result and the CLI args that drove this run.
    ///
    /// `diff_path` is the path that was written, or `None` if `--no-diff`.
    pub fn from_compare(result: &CompareResult, args: &Args, diff_path: Option<PathBuf>) -> Self {
        Self {
            score: result.score,
            threshold: args.threshold,
            passed: result.score >= args.threshold,
            width: result.width,
            height: result.height,
            diff_path,
        }
    }

    /// Format as a two-line human-readable summary.
    ///
    /// Example: `"score: 0.9958 (99.58% similar)\ndiff:  diff.png\n"`
    pub fn to_human(&self) -> String {
        let diff_display = match &self.diff_path {
            Some(path) => path.display().to_string(),
            None => "(skipped)".to_string(),
        };
        format!(
            "score: {:.4} ({:.2}% similar)\ndiff:  {diff_display}\n",
            self.score,
            self.score * 100.0,
        )
    }

    /// Format as a compact JSON line ending with `\n`.
    ///
    /// Uses compact (non-pretty) serialization suitable for CI consumption.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string(self).expect("Report serialization is infallible");
        s.push('\n');
        s
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

        let report = Report::from_compare(&result, &args, diff_path.clone());

        assert!((report.score - 0.9758).abs() < f64::EPSILON);
        assert!((report.threshold - 0.95).abs() < f64::EPSILON);
        assert!(report.passed);
        assert_eq!(report.width, 800);
        assert_eq!(report.height, 600);
        assert_eq!(report.diff_path, diff_path);
    }

    #[test]
    fn from_compare_should_set_passed_false_when_score_below_threshold() {
        let result = CompareResult::test_fixture(0.5, 10, 10);
        let args = make_args(0.99, false);

        let report = Report::from_compare(&result, &args, None);

        assert!(!report.passed);
    }

    #[test]
    fn to_human_should_format_score_and_diff_path() {
        let report = Report {
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            width: 100,
            height: 200,
            diff_path: Some(PathBuf::from("diff.png")),
        };

        let output = report.to_human();

        assert_eq!(output, "score: 0.9958 (99.58% similar)\ndiff:  diff.png\n");
    }

    #[test]
    fn to_human_should_show_skipped_when_no_diff_path() {
        let report = Report {
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            width: 100,
            height: 200,
            diff_path: None,
        };

        let output = report.to_human();

        assert_eq!(output, "score: 0.9958 (99.58% similar)\ndiff:  (skipped)\n");
    }

    #[test]
    fn to_json_should_round_trip_with_expected_values() {
        let report = Report {
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            width: 100,
            height: 200,
            diff_path: Some(PathBuf::from("diff.png")),
        };

        let json_str = report.to_json();
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("JSON should be valid");

        assert!((parsed["score"].as_f64().unwrap() - 0.9958).abs() < 1e-10);
        assert!((parsed["threshold"].as_f64().unwrap() - 0.99).abs() < 1e-10);
        assert_eq!(parsed["passed"].as_bool().unwrap(), true);
        assert_eq!(parsed["width"].as_u64().unwrap(), 100);
        assert_eq!(parsed["height"].as_u64().unwrap(), 200);
        assert_eq!(parsed["diff_path"].as_str().unwrap(), "diff.png");
    }

    #[test]
    fn to_json_should_omit_diff_path_when_none() {
        let report = Report {
            score: 0.9958,
            threshold: 0.99,
            passed: true,
            width: 100,
            height: 200,
            diff_path: None,
        };

        let json_str = report.to_json();
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("JSON should be valid");

        assert!(
            parsed.get("diff_path").is_none(),
            "diff_path key should not be present in JSON when None"
        );
    }
}
