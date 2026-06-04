use image::{Rgba, RgbaImage};
use image_compare::Similarity;

/// Renders a red-overlay diff image: pixels where the two screenshots disagree
/// are tinted with `color` over the implementation screenshot, with alpha
/// proportional to the per-pixel disagreement.
///
/// `gain` boosts visibility (default 4.0). Higher gain → smaller differences
/// become more visible.
#[expect(dead_code, reason = "called by CLI wiring in a later task (T4)")]
pub fn render(
    impl_image: &RgbaImage,
    similarity: &Similarity,
    color: Rgba<u8>,
    gain: f32,
) -> RgbaImage {
    let color_map = similarity.image.to_color_map().to_rgba8();

    debug_assert_eq!(
        impl_image.dimensions(),
        color_map.dimensions(),
        "overlay::render: impl image and similarity dims must match"
    );

    let (width, height) = impl_image.dimensions();
    let mut output = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let impl_px = impl_image.get_pixel(x, y);
            let sim_px = color_map.get_pixel(x, y);

            let min_sim_u8 = sim_px.0.iter().copied().min().unwrap_or(0);
            let diff = 1.0 - (min_sim_u8 as f32 / 255.0);
            let alpha_factor = (diff * gain).clamp(0.0, 1.0);

            let out_r = lerp(impl_px[0], color[0], alpha_factor);
            let out_g = lerp(impl_px[1], color[1], alpha_factor);
            let out_b = lerp(impl_px[2], color[2], alpha_factor);
            let out_a = impl_px[3];

            output.put_pixel(x, y, Rgba([out_r, out_g, out_b, out_a]));
        }
    }

    output
}

/// Linear interpolation between `a` and `b` by `t` (in `[0.0, 1.0]`).
///
/// `t == 0.0` returns `a`; `t == 1.0` returns `b`.
fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let a_f = a as f32;
    let b_f = b as f32;
    (a_f + (b_f - a_f) * t).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba, RgbaImage};
    use image_compare::Similarity;
    use image_compare::prelude::RGBASimilarityImage;

    fn make_similarity(width: u32, height: u32, fill: Rgba<f32>) -> Similarity {
        let sim_image: RGBASimilarityImage = ImageBuffer::from_pixel(width, height, fill);
        Similarity {
            score: 1.0,
            image: sim_image.into(),
        }
    }

    fn make_impl_image() -> RgbaImage {
        // 2x2 image with four distinct pixels
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        img.put_pixel(1, 0, Rgba([40, 50, 60, 200]));
        img.put_pixel(0, 1, Rgba([70, 80, 90, 128]));
        img.put_pixel(1, 1, Rgba([100, 110, 120, 64]));
        img
    }

    /// When every similarity channel is 1.0 (color_map → u8 255 → diff = 0 → alpha_factor = 0),
    /// the output must equal the implementation image exactly.
    #[test]
    fn identical_similarity_should_return_impl_image_unchanged() {
        let impl_image = make_impl_image();
        let similarity = make_similarity(2, 2, Rgba([1.0f32, 1.0, 1.0, 1.0]));

        let result = render(&impl_image, &similarity, Rgba([255, 0, 0, 255]), 4.0);

        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(
                    result.get_pixel(x, y),
                    impl_image.get_pixel(x, y),
                    "pixel ({x},{y}) should be unchanged with perfect similarity"
                );
            }
        }
    }

    /// When every similarity channel is 0.0 (color_map → u8 0 → diff = 1 → alpha_factor = 1),
    /// the output RGB channels must equal the overlay color. Alpha is taken from impl_image.
    #[test]
    fn zero_similarity_should_produce_fully_colored_overlay() {
        let impl_image = make_impl_image();
        let similarity = make_similarity(2, 2, Rgba([0.0f32, 0.0, 0.0, 0.0]));
        let overlay_color = Rgba([255, 0, 0, 255]);

        let result = render(&impl_image, &similarity, overlay_color, 4.0);

        for y in 0..2 {
            for x in 0..2 {
                let out = result.get_pixel(x, y);
                let impl_px = impl_image.get_pixel(x, y);
                assert_eq!(out[0], 255, "pixel ({x},{y}) red channel should be 255");
                assert_eq!(out[1], 0, "pixel ({x},{y}) green channel should be 0");
                assert_eq!(out[2], 0, "pixel ({x},{y}) blue channel should be 0");
                assert_eq!(
                    out[3], impl_px[3],
                    "pixel ({x},{y}) alpha should match impl image"
                );
            }
        }
    }

    /// Partial similarity: min_sim_u8 ≈ 191 (0.75 * 255), diff ≈ 0.251.
    /// With gain=4.0: alpha_factor = clamp(0.251 * 4, 0, 1) = 1.0 → fully colored.
    /// With gain=1.0: alpha_factor ≈ 0.251 → lerp(0, 255, 0.251) ≈ 64.
    #[test]
    fn partial_similarity_should_scale_blend_with_gain() {
        let impl_image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        // 0.75 * 255 = 191.25 → truncated to 191 in to_color_map_rgba
        let similarity = make_similarity(1, 1, Rgba([0.75f32, 0.75, 0.75, 0.75]));
        let overlay_color = Rgba([255, 0, 0, 255]);

        // With gain=4.0, alpha_factor should clamp to 1.0 → fully red
        let result_high_gain = render(&impl_image, &similarity, overlay_color, 4.0);
        let px_high = result_high_gain.get_pixel(0, 0);
        assert_eq!(
            px_high[0], 255,
            "gain=4.0: red channel should be 255 (fully overlaid)"
        );
        assert_eq!(px_high[1], 0, "gain=4.0: green channel should be 0");
        assert_eq!(px_high[2], 0, "gain=4.0: blue channel should be 0");
        assert_eq!(px_high[3], 255, "alpha should be preserved");

        // With gain=1.0: diff = 1 - 191/255 ≈ 0.251, lerp(0, 255, 0.251) ≈ 64
        let result_low_gain = render(&impl_image, &similarity, overlay_color, 1.0);
        let px_low = result_low_gain.get_pixel(0, 0);
        let expected_r = (255.0_f32 * (1.0 - 191.0 / 255.0)).round() as u8;
        assert!(
            px_low[0].abs_diff(expected_r) <= 1,
            "gain=1.0: red channel {} should be ≈{} (±1 for rounding)",
            px_low[0],
            expected_r
        );
        assert_eq!(px_low[1], 0, "gain=1.0: green channel should be 0");
        assert_eq!(px_low[2], 0, "gain=1.0: blue channel should be 0");
        assert_eq!(px_low[3], 255, "alpha should be preserved");
    }
}
