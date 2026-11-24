use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

/// Exit codes and what they mean. Printed in `--help` and in `kuk schema`.
pub const EXIT_CODES: [(u8, &str); 4] = [
    (0, "ok"),
    (1, "score below --threshold (only with --fail)"),
    (2, "usage or runtime error"),
    (3, "dimension mismatch; the report is on stdout"),
];

const EXAMPLES: &str = "\
Examples:
  kuk design.png impl.png
  kuk design.png impl.png --format toon
  kuk design.png impl.png --json --fail --threshold 0.995
  kuk design.png impl.png --no-diff
  kuk completions zsh > ~/.zfunc/_kuk
  kuk schema";

/// Output format selector. CLI accepts only lowercase variants.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[clap(rename_all = "lower")]
pub enum OutputFormat {
    /// Default. Multi-line text for people.
    #[value(alias = "human")]
    Text,
    /// Compact single-line JSON.
    Json,
    /// TOON (Token-Oriented Object Notation), compact and agent-friendly.
    Toon,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Print a shell completion script to stdout.
    Completions {
        /// Shell to generate the script for.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Describe every command, flag, exit code and output shape as JSON.
    Schema,
}

#[derive(Parser, Debug)]
#[command(
    name = "kuk",
    version,
    about = "Compare two webpage screenshots and emit a similarity score + red-overlay diff PNG",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to the design (reference) image.
    #[arg(required = true, value_parser = parse_path)]
    pub design: Option<PathBuf>,

    /// Path to the implementation image.
    #[arg(value_name = "IMPL", required = true, value_parser = parse_path)]
    pub implementation: Option<PathBuf>,

    /// Where to write the diff PNG.
    #[arg(short, long, default_value = "diff.png", value_name = "PATH", value_parser = parse_path)]
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

    /// Output format. `text` is multi-line text, `json` is compact JSON,
    /// `toon` is TOON (token-efficient for agent contexts).
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, value_name = "FORMAT")]
    pub format: OutputFormat,

    /// Shorthand for `--format json`.
    #[arg(long, conflicts_with = "format")]
    pub json: bool,

    /// Skip writing the diff PNG.
    #[arg(long)]
    pub no_diff: bool,
}

impl Args {
    /// The format to print in, with `--json` folded in.
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format
        }
    }
}

/// Paths come from agents as well as people, so control characters are refused.
fn parse_path(raw: &str) -> Result<PathBuf, String> {
    if raw.chars().any(char::is_control) {
        return Err("path contains a control character".to_string());
    }
    Ok(PathBuf::from(raw))
}

/// The clap command with examples and exit codes appended to its help.
pub(crate) fn command() -> clap::Command {
    let codes: String = EXIT_CODES
        .iter()
        .map(|(code, meaning)| format!("\n  {code}  {meaning}"))
        .collect();
    Args::command().after_help(format!("{EXAMPLES}\n\nExit codes:{codes}"))
}

pub(crate) fn parse() -> Args {
    let matches = command().get_matches();
    Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defaults_should_be_set_correctly() {
        let args = Args::try_parse_from(["kuk", "a.png", "b.png"]).expect("parse should succeed");

        assert_eq!(args.design, Some(PathBuf::from("a.png")));
        assert_eq!(args.implementation, Some(PathBuf::from("b.png")));
        assert_eq!(args.output, PathBuf::from("diff.png"));
        assert!((args.threshold - 0.99).abs() < f64::EPSILON);
        assert!((args.gain - 4.0).abs() < f32::EPSILON);
        assert!(!args.fail);
        assert_eq!(args.output_format(), OutputFormat::Text);
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
        assert_eq!(args.output_format(), OutputFormat::Json);
        assert!(args.no_diff);
    }

    #[test]
    fn format_toon_should_parse() {
        let args = Args::try_parse_from(["kuk", "a.png", "b.png", "--format", "toon"])
            .expect("parse should succeed");
        assert_eq!(args.output_format(), OutputFormat::Toon);
    }

    #[test]
    fn format_human_should_stay_an_alias_for_text() {
        let args = Args::try_parse_from(["kuk", "a.png", "b.png", "--format", "human"])
            .expect("parse should succeed");
        assert_eq!(args.output_format(), OutputFormat::Text);
    }

    #[test]
    fn json_flag_should_select_json() {
        let args = Args::try_parse_from(["kuk", "a.png", "b.png", "--json"])
            .expect("parse should succeed");
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn json_flag_with_format_should_be_rejected() {
        let result = Args::try_parse_from(["kuk", "a.png", "b.png", "--json", "--format", "toon"]);
        assert!(result.is_err(), "--json and --format must conflict");
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

    #[test]
    fn path_with_control_character_should_be_rejected() {
        let result = Args::try_parse_from(["kuk", "a\u{7}.png", "b.png"]);
        assert!(
            result.is_err(),
            "control characters in paths must be rejected"
        );
    }

    #[test]
    fn subcommand_should_parse_without_image_arguments() {
        let args = Args::try_parse_from(["kuk", "schema"]).expect("parse should succeed");
        assert!(matches!(args.command, Some(Command::Schema)));
    }
}
