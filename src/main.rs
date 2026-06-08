mod cli;
mod compare;
mod error;
mod overlay;
mod report;

use image::Rgba;
use std::process::ExitCode;

use crate::cli::{Args, OutputFormat};
use crate::error::PeepError;
use crate::report::Report;

const OVERLAY_COLOR: Rgba<u8> = Rgba([255, 0, 0, 255]);

#[derive(serde::Serialize)]
struct ToonMismatch<'a> {
    sources: [ToonSource<'a>; 2],
    dims_match: bool,
    delta: ToonDelta,
    error: &'static str,
}

#[derive(serde::Serialize)]
struct ToonSource<'a> {
    label: &'a str,
    path: &'a std::path::Path,
    width: u32,
    height: u32,
}

#[derive(serde::Serialize)]
struct ToonDelta {
    width: i64,
    height: i64,
}

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
            writeln!(out, "kuk: dimension mismatch")?;
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
            let shim = ToonMismatch {
                sources: [
                    ToonSource {
                        label: "a",
                        path: design_path,
                        width: aw,
                        height: ah,
                    },
                    ToonSource {
                        label: "b",
                        path: impl_path,
                        width: bw,
                        height: bh,
                    },
                ],
                dims_match: false,
                delta: ToonDelta {
                    width: dw,
                    height: dh,
                },
                error: "dimension_mismatch",
            };
            let mut s = toon_format::encode_default(&shim)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            if !s.ends_with('\n') {
                s.push('\n');
            }
            out.write_all(s.as_bytes())?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = cli::parse();
    match run(&args) {
        Ok(report) => {
            if let Err(e) = print_report(&report, args.format) {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
            if args.fail && !report.passed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
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
    Ok(Report::from_compare(
        &result,
        args,
        args.design.clone(),
        args.implementation.clone(),
        diff_path,
    ))
}

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

    use crate::cli::OutputFormat;

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
