mod cli;
mod compare;
mod error;
mod overlay;
mod report;
mod schema;

use image::Rgba;
use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use crate::cli::{Args, Command};
use crate::error::KukError;
use crate::report::{Mismatch, Report};

/// Opaque red used to highlight per-pixel deltas on the diff PNG.
const OVERLAY_COLOR: Rgba<u8> = Rgba([255, 0, 0, 255]);

fn main() -> ExitCode {
    let args = cli::parse();
    let outcome = match &args.command {
        Some(command) => describe(command),
        None => compare(&args),
    };
    outcome.unwrap_or_else(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })
}

/// Run a comparison and print its report. `Ok` carries the exit code of a run
/// that produced a report; `Err` is a failure to print one.
fn compare(args: &Args) -> Result<ExitCode, Box<dyn Error>> {
    let (Some(design), Some(implementation)) = (&args.design, &args.implementation) else {
        unreachable!("clap requires both images when no subcommand is given");
    };
    let format = args.output_format();
    match run(args, design, implementation) {
        Ok(report) => {
            print(&report.render(format)?)?;
            Ok(if args.fail && !report.passed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
        Err(KukError::DimMismatch {
            width_a,
            height_a,
            width_b,
            height_b,
        }) => {
            let mismatch = Mismatch::new(
                design.clone(),
                implementation.clone(),
                (width_a, height_a),
                (width_b, height_b),
            );
            print(&mismatch.render(format)?)?;
            Ok(ExitCode::from(3))
        }
        Err(e) => {
            eprintln!("error: {e}");
            // Walk the error source chain for context, if any.
            let mut source = e.source();
            while let Some(s) = source {
                eprintln!("  caused by: {s}");
                source = s.source();
            }
            if let Some(hint) = e.hint() {
                eprintln!("  hint: {hint}");
            }
            Ok(ExitCode::from(2))
        }
    }
}

/// The commands that describe the tool instead of comparing images.
fn describe(command: &Command) -> Result<ExitCode, Box<dyn Error>> {
    match command {
        Command::Completions { shell } => {
            clap_complete::generate(*shell, &mut cli::command(), "kuk", &mut std::io::stdout());
        }
        Command::Schema => {
            let mut json = serde_json::to_string_pretty(&schema::build())?;
            json.push('\n');
            print(&json)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn run(args: &Args, design: &Path, implementation: &Path) -> Result<Report, KukError> {
    let result = compare::run(design, implementation)?;
    let diff_path = if args.no_diff {
        None
    } else {
        let diff = overlay::render(
            &result.impl_image,
            &result.similarity,
            OVERLAY_COLOR,
            args.gain,
        );
        diff.save(&args.output).map_err(|e| KukError::DiffWrite {
            path: args.output.clone(),
            source: e,
        })?;
        Some(args.output.clone())
    };
    Ok(Report::from_compare(
        &result,
        args,
        design.to_path_buf(),
        implementation.to_path_buf(),
        diff_path,
    ))
}

fn print(rendered: &str) -> std::io::Result<()> {
    std::io::stdout().lock().write_all(rendered.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;
    use image::{Rgba, RgbaImage};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn save_rgba_png(img: &RgbaImage) -> NamedTempFile {
        let file = NamedTempFile::with_suffix(".png").expect("tempfile");
        img.save(file.path()).expect("save png");
        file
    }

    fn make_args(design: PathBuf, implementation: PathBuf, output: PathBuf) -> Args {
        Args {
            command: None,
            design: Some(design),
            implementation: Some(implementation),
            output,
            threshold: 0.99,
            gain: 4.0,
            fail: false,
            format: OutputFormat::Text,
            json: false,
            no_diff: false,
        }
    }

    #[test]
    fn run_with_identical_images_should_pass_and_write_diff_png() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([100, 150, 200, 255]));
        let file_a = save_rgba_png(&img);
        let file_b = save_rgba_png(&img);
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let output = tmp_dir.path().join("diff.png");

        let args = make_args(
            file_a.path().to_path_buf(),
            file_b.path().to_path_buf(),
            output.clone(),
        );

        let report = run(
            &args,
            args.design.as_deref().unwrap(),
            args.implementation.as_deref().unwrap(),
        )
        .expect("run should succeed with identical images");

        assert!(
            report.passed,
            "identical images should pass at threshold 0.99"
        );
        assert_eq!(
            report.diff_path,
            Some(output.clone()),
            "diff_path should point to the output file"
        );
        assert!(output.exists(), "diff PNG should have been written to disk");
    }

    #[test]
    fn run_with_no_diff_flag_should_return_none_diff_path_and_not_write_file() {
        let img = RgbaImage::from_pixel(4, 4, Rgba([200, 100, 50, 255]));
        let file_a = save_rgba_png(&img);
        let file_b = save_rgba_png(&img);
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let output = tmp_dir.path().join("diff.png");

        let mut args = make_args(
            file_a.path().to_path_buf(),
            file_b.path().to_path_buf(),
            output.clone(),
        );
        args.no_diff = true;

        let report = run(
            &args,
            args.design.as_deref().unwrap(),
            args.implementation.as_deref().unwrap(),
        )
        .expect("run should succeed with no_diff flag");

        assert_eq!(
            report.diff_path, None,
            "diff_path should be None when --no-diff is set"
        );
        assert!(
            !output.exists(),
            "diff PNG must not be written when --no-diff is set"
        );
    }
}
