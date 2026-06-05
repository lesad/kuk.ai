use assert_cmd::Command;
use image::{DynamicImage, ImageBuffer, Rgba};
use predicates::str::contains;
use std::path::Path;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

fn write_solid_png(path: &Path, w: u32, h: u32, color: Rgba<u8>) {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(w, h, color);
    DynamicImage::ImageRgba8(img)
        .save(path)
        .expect("write fixture PNG");
}

fn write_png_with_diff_block(
    path: &Path,
    w: u32,
    h: u32,
    bg: Rgba<u8>,
    fg: Rgba<u8>,
    block_xy: (u32, u32),
    block_size: u32,
) {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(w, h, bg);
    for dy in 0..block_size {
        for dx in 0..block_size {
            img.put_pixel(block_xy.0 + dx, block_xy.1 + dy, fg);
        }
    }
    DynamicImage::ImageRgba8(img)
        .save(path)
        .expect("write fixture PNG");
}

fn peep() -> Command {
    Command::cargo_bin("peep").expect("peep binary not found")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn identical_pngs_should_exit_0_and_score_near_1_and_write_diff() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 32, 32, Rgba([255, 0, 0, 255]));
    write_solid_png(&b, 32, 32, Rgba([255, 0, 0, 255]));

    peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .assert()
        .success()
        .stdout(contains("score: 1.0000"));

    assert!(out.exists(), "diff PNG should be written");
}

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

#[test]
fn different_pngs_without_fail_flag_should_exit_0_with_score_below_threshold() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let out = dir.path().join("diff.png");

    // 32x32 background; 16x16 contrasting block at (0,0) — large enough to guarantee score < 0.99
    write_solid_png(&a, 32, 32, Rgba([255, 255, 255, 255]));
    write_png_with_diff_block(
        &b,
        32,
        32,
        Rgba([255, 255, 255, 255]),
        Rgba([0, 0, 0, 255]),
        (0, 0),
        16,
    );

    let output = peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--format").arg("json")
        .assert()
        // No --fail flag: must exit 0 even though score < threshold
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("stdout should be valid JSON");

    assert_eq!(
        json["passed"].as_bool(),
        Some(false),
        "passed should be false for significantly different images"
    );
    assert!(
        json["score"].as_f64().unwrap() < 0.99,
        "expected score < 0.99 for significantly different images, got {}",
        json["score"]
    );
    assert!(out.exists(), "diff PNG should be written even on low score");
}

#[test]
fn different_pngs_with_fail_and_threshold_should_exit_1() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 32, 32, Rgba([255, 255, 255, 255]));
    write_png_with_diff_block(
        &b,
        32,
        32,
        Rgba([255, 255, 255, 255]),
        Rgba([0, 0, 0, 255]),
        (0, 0),
        16,
    );

    peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--fail")
        .arg("--threshold")
        .arg("0.99")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn different_pngs_with_fail_and_json_should_exit_1_with_passed_false() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let out = dir.path().join("diff.png");

    write_solid_png(&a, 32, 32, Rgba([255, 255, 255, 255]));
    write_png_with_diff_block(
        &b,
        32,
        32,
        Rgba([255, 255, 255, 255]),
        Rgba([0, 0, 0, 255]),
        (0, 0),
        16,
    );

    let output = peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--fail")
        .arg("--threshold")
        .arg("0.99")
        .arg("--format").arg("json")
        .assert()
        .failure()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("stdout should be valid JSON");

    assert_eq!(
        json["passed"].as_bool(),
        Some(false),
        "passed should be false when score < threshold"
    );
}

#[test]
fn no_diff_flag_should_not_write_diff_file_and_stdout_shows_skipped() {
    let dir = tempdir().expect("tempdir");
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    let out = dir.path().join("should-not-exist.png");

    write_solid_png(&a, 32, 32, Rgba([0, 128, 255, 255]));
    write_solid_png(&b, 32, 32, Rgba([0, 128, 255, 255]));

    peep()
        .arg(&a)
        .arg(&b)
        .arg("--output")
        .arg(&out)
        .arg("--no-diff")
        .assert()
        .success()
        .stdout(contains("(skipped)"));

    assert!(
        !out.exists(),
        "diff PNG must not be written when --no-diff is set"
    );
}

#[test]
fn missing_input_file_should_exit_2_with_error_on_stderr() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("nonexistent.png");
    let other = dir.path().join("other.png");

    write_solid_png(&other, 4, 4, Rgba([0, 0, 0, 255]));

    peep()
        .arg(&missing)
        .arg(&other)
        .assert()
        .failure()
        .code(2)
        .stderr(contains("failed to load image"))
        .stderr(contains("nonexistent.png"));
}

#[test]
fn dimension_mismatch_should_exit_2_with_sizes_in_stderr() {
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
        .code(2)
        .stderr(contains("dimension mismatch"))
        .stderr(contains("4x4"))
        .stderr(contains("8x8"));
}

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

#[test]
fn version_flag_should_exit_0_and_print_crate_version() {
    peep()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

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
