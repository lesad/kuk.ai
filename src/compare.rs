use std::path::Path;

use image::RgbaImage;
use image_compare::Similarity;

use crate::error::PeepError;

/// The result of comparing a design image against an implementation screenshot.
#[derive(Debug)]
pub struct CompareResult {
    /// Similarity score in `[0.0, 1.0]` where `1.0` means pixel-perfect identical.
    pub score: f64,
    /// Full similarity result from `image_compare`, containing the per-pixel similarity map.
    /// Access the image via `similarity.image`.
    pub similarity: Similarity,
    /// The implementation image (RGBA8), useful for alpha-blending a diff overlay later.
    pub impl_image: RgbaImage,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Load both images, assert they share dimensions, then run a hybrid RGBA comparison.
///
/// # Errors
///
/// - [`PeepError::ImageLoad`] if either file cannot be opened or decoded.
/// - [`PeepError::DimMismatch`] if the two images have different dimensions.
/// - [`PeepError::Compare`] if `image_compare` returns an error.
pub fn run(design: &Path, implementation: &Path) -> Result<CompareResult, PeepError> {
    let design_rgba = load_rgba(design)?;
    let impl_rgba = load_rgba(implementation)?;

    let (dw, dh) = design_rgba.dimensions();
    let (iw, ih) = impl_rgba.dimensions();
    if (dw, dh) != (iw, ih) {
        return Err(PeepError::DimMismatch {
            width_a: dw,
            height_a: dh,
            width_b: iw,
            height_b: ih,
        });
    }

    let similarity = image_compare::rgba_hybrid_compare(&design_rgba, &impl_rgba)?;
    let score = similarity.score;

    Ok(CompareResult {
        score,
        similarity,
        width: iw,
        height: ih,
        impl_image: impl_rgba,
    })
}

fn load_rgba(path: &Path) -> Result<RgbaImage, PeepError> {
    image::open(path)
        .map_err(|e| PeepError::ImageLoad {
            path: path.to_path_buf(),
            source: e,
        })
        .map(|img| img.into_rgba8())
}

#[cfg(test)]
impl CompareResult {
    /// Construct a minimal `CompareResult` for use in unit tests that only care
    /// about `score`, `width`, and `height` — not the full similarity image.
    pub(crate) fn test_fixture(score: f64, width: u32, height: u32) -> Self {
        use image::{ImageBuffer, Rgba, RgbaImage};
        use image_compare::prelude::RGBASimilarityImage;

        let sim_image: RGBASimilarityImage =
            ImageBuffer::from_pixel(width, height, Rgba([0.0f32, 0.0, 0.0, 1.0]));
        let similarity = Similarity {
            score,
            image: sim_image.into(),
        };
        let impl_image = RgbaImage::new(width, height);
        Self {
            score,
            similarity,
            impl_image,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn save_rgba_to_temp(img: &RgbaImage) -> NamedTempFile {
        let file = NamedTempFile::with_suffix(".png").expect("tempfile");
        img.save(file.path()).expect("save");
        file
    }

    fn solid_rgba_image(width: u32, height: u32, color: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = color;
        }
        img
    }

    #[test]
    fn identical_pngs_should_score_near_one() {
        let img = solid_rgba_image(4, 4, Rgba([100, 150, 200, 255]));
        let file_a = save_rgba_to_temp(&img);
        let file_b = save_rgba_to_temp(&img);

        let result = run(file_a.path(), file_b.path()).expect("compare should succeed");

        assert!(
            result.score > 0.999,
            "expected score > 0.999, got {}",
            result.score
        );
        assert_eq!(result.width, 4);
        assert_eq!(result.height, 4);
    }

    #[test]
    fn dim_mismatch_should_return_err_dim_mismatch() {
        let img_4x4 = solid_rgba_image(4, 4, Rgba([255, 0, 0, 255]));
        let img_8x8 = solid_rgba_image(8, 8, Rgba([0, 255, 0, 255]));
        let file_a = save_rgba_to_temp(&img_4x4);
        let file_b = save_rgba_to_temp(&img_8x8);

        let err = run(file_a.path(), file_b.path()).expect_err("should fail with dim mismatch");

        match err {
            PeepError::DimMismatch {
                width_a,
                height_a,
                width_b,
                height_b,
            } => {
                assert_eq!((width_a, height_a), (4, 4));
                assert_eq!((width_b, height_b), (8, 8));
            }
            other => panic!("expected DimMismatch, got: {other}"),
        }
    }

    #[test]
    fn missing_file_should_return_err_image_load() {
        let bad_path = PathBuf::from("/nonexistent/path/image.png");
        let img = solid_rgba_image(4, 4, Rgba([0, 0, 0, 255]));
        let good_file = save_rgba_to_temp(&img);

        let err = run(&bad_path, good_file.path()).expect_err("should fail with image load error");

        match err {
            PeepError::ImageLoad { path, .. } => {
                assert_eq!(path, bad_path);
            }
            other => panic!("expected ImageLoad, got: {other}"),
        }
    }
}
