mod cli;
mod compare;
mod error;
mod overlay;
mod report;

use image::Rgba;
use std::process::ExitCode;

use crate::cli::Args;
use crate::error::PeepError;
use crate::report::Report;

const OVERLAY_COLOR: Rgba<u8> = Rgba([255, 0, 0, 255]);

fn main() -> ExitCode {
    let args = cli::parse();
    match run(&args) {
        Ok(report) => {
            if let Err(e) = print_report(&report, args.json) {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
            if args.fail && !report.passed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
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
    }
}

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
    Ok(Report::from_compare(&result, args, diff_path))
}

fn print_report(report: &Report, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if json {
        out.write_all(report.to_json()?.as_bytes())?;
    } else {
        out.write_all(report.to_human().as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            design,
            implementation,
            output,
            threshold: 0.99,
            gain: 4.0,
            fail: false,
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

        let report = run(&args).expect("run should succeed with identical images");

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

        let report = run(&args).expect("run should succeed with no_diff flag");

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
