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
    name = "kuk",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defaults_should_be_set_correctly() {
        let args = Args::try_parse_from(["kuk", "a.png", "b.png"]).expect("parse should succeed");

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
            "kuk",
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
        let args = Args::try_parse_from(["kuk", "a.png", "b.png", "--format", "toon"])
            .expect("parse should succeed");
        assert_eq!(args.format, OutputFormat::Toon);
    }

    #[test]
    fn format_uppercase_should_be_rejected() {
        let result = Args::try_parse_from(["kuk", "a.png", "b.png", "--format", "JSON"]);
        assert!(result.is_err(), "uppercase format values must be rejected");
    }

    #[test]
    fn missing_implementation_argument_should_error() {
        let result = Args::try_parse_from(["kuk", "a.png"]);
        assert!(result.is_err(), "expected parse error when IMPL is missing");
    }
}
