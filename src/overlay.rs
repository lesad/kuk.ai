use image::{Rgba, RgbaImage};
use image_compare::Similarity;

/// Renders a tinted overlay diff image. Each pixel's overlay opacity is
/// proportional to the largest per-channel difference (R, G, B in the hybrid
/// similarity image), down-weighted by the per-pixel visibility (alpha_vis).
///
/// In `rgba_hybrid_compare`'s `RGBASimilarityImage` (hybrid mode), the encoding is:
/// - R = `1 - y_ssim_similarity` (0 = luma match, 1 = max luma diff)
/// - G = `1 - u_rms_similarity`  (0 = U chroma match, 1 = max U diff)
/// - B = `1 - v_rms_similarity`  (0 = V chroma match, 1 = max V diff)
/// - A = `alpha_vis` ∈ [0.1, 1.0] — visibility weight based on mean alpha of
///   source pixels; **not** a diff value. High = highly visible region.
///
/// After `to_color_map().to_rgba8()` each channel is `(value * 255) as u8`.
/// So R/G/B: 0 = match, 255 = max diff. A: 25..=255 = visibility.
///
/// `gain` amplifies the weighted diff before clamping to [0, 1]. Typical value
/// 4.0; higher = exaggerate small diffs.
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

            // R, G, B are diff channels (0 = no diff, 255 = max diff).
            // A is a visibility weight — intentionally excluded from diff max.
            let [r, g, b, a_vis] = sim_px.0;
            let max_diff_u8 = r.max(g).max(b);
            let diff = max_diff_u8 as f32 / 255.0;
            let visibility = a_vis as f32 / 255.0;
            let weighted_diff = diff * visibility;
            let alpha_factor = (weighted_diff * gain).clamp(0.0, 1.0);

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
    use image_compare::prelude::RGBASimilarityImage;

    fn make_similarity(width: u32, height: u32, fill: Rgba<f32>) -> image_compare::Similarity {
        let sim_image: RGBASimilarityImage = ImageBuffer::from_pixel(width, height, fill);
        image_compare::Similarity {
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

    /// No difference: R=G=B=0.0 means zero diff; visibility=1.0.
    /// alpha_factor = 0 → output equals impl image byte-for-byte.
    #[test]
    fn no_difference_should_return_impl_image_unchanged() {
        let impl_image = make_impl_image();
        // Correct hybrid encoding: R/G/B=0.0 = no diff; A=1.0 = full visibility
        let similarity = make_similarity(2, 2, Rgba([0.0f32, 0.0, 0.0, 1.0]));

        let result = render(&impl_image, &similarity, Rgba([255, 0, 0, 255]), 4.0);

        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(
                    result.get_pixel(x, y),
                    impl_image.get_pixel(x, y),
                    "pixel ({x},{y}) should be unchanged when diff channels are 0"
                );
            }
        }
    }

    /// Max difference, full visibility: R=G=B=1.0, A=1.0.
    /// max_diff=1.0, visibility=1.0, weighted=1.0, alpha_factor=1.0 →
    /// output RGB equals overlay color; output alpha equals impl alpha.
    #[test]
    fn max_difference_full_visibility_should_produce_fully_colored_overlay() {
        let impl_image = make_impl_image();
        // Correct hybrid encoding: R/G/B=1.0 = max diff; A=1.0 = full visibility
        let similarity = make_similarity(2, 2, Rgba([1.0f32, 1.0, 1.0, 1.0]));
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

    /// Partial diff with gain scaling.
    ///
    /// Similarity pixel: R=G=B=0.25, A=1.0 (diff≈0.25, visibility=1.0).
    ///
    /// Note: `f32` 0.25 converts to `u8` 63 via `(0.25 * 255) as u8`. So:
    /// - gain=4.0 → weighted_diff=63/255≈0.247, alpha_factor≈0.988 → lerp≈252 (≥250)
    /// - gain=1.0 → alpha_factor≈0.247 → lerp(0,255,0.247)≈63
    #[test]
    fn partial_diff_should_scale_blend_with_gain() {
        let impl_image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        // diff≈0.25, visibility=1.0: R/G/B=0.25 → u8 63, A=1.0 → u8 255
        let similarity = make_similarity(1, 1, Rgba([0.25f32, 0.25, 0.25, 1.0]));
        let overlay_color = Rgba([255, 0, 0, 255]);

        // gain=4.0: 63/255*4 ≈ 0.988 → nearly fully saturated (≥250)
        let result_high_gain = render(&impl_image, &similarity, overlay_color, 4.0);
        let px_high = result_high_gain.get_pixel(0, 0);
        assert!(
            px_high[0] >= 250,
            "gain=4.0: red channel {} should be ≥250 (nearly fully overlaid)",
            px_high[0]
        );
        assert_eq!(px_high[1], 0, "gain=4.0: green channel should be 0");
        assert_eq!(px_high[2], 0, "gain=4.0: blue channel should be 0");
        assert_eq!(px_high[3], 255, "alpha should be preserved");

        // gain=1.0: alpha_factor ≈ 63/255 ≈ 0.247 → lerp(0,255,0.247) ≈ 63
        let result_low_gain = render(&impl_image, &similarity, overlay_color, 1.0);
        let px_low = result_low_gain.get_pixel(0, 0);
        let expected_r: u8 = 63;
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

    /// Visibility weighting: diff=0.25 but a_vis=0.4 down-weights the overlay.
    ///
    /// Quantized: diff_u8=63, vis_u8=102.
    /// weighted_diff = (63/255) * (102/255) ≈ 0.099; gain=4.0 → alpha≈0.395 → lerp≈101.
    #[test]
    fn partial_diff_low_visibility_should_reduce_overlay_intensity() {
        let impl_image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        // diff=0.25 → u8 63, visibility=0.4 → u8 102
        let similarity = make_similarity(1, 1, Rgba([0.25f32, 0.25, 0.25, 0.4]));
        let overlay_color = Rgba([255, 0, 0, 255]);

        // (63/255) * (102/255) * 4 ≈ 0.395 → lerp(0, 255, 0.395) ≈ 101
        let result = render(&impl_image, &similarity, overlay_color, 4.0);
        let px = result.get_pixel(0, 0);
        let expected_r: u8 = 101;
        assert!(
            px[0].abs_diff(expected_r) <= 1,
            "low visibility: red channel {} should be ≈{} (±1 for rounding)",
            px[0],
            expected_r
        );
        assert_eq!(px[1], 0, "green channel should be 0");
        assert_eq!(px[2], 0, "blue channel should be 0");
        assert_eq!(px[3], 255, "alpha should be preserved");
    }

    /// Asymmetric diff channels: only B channel is non-zero (B=1.0).
    ///
    /// max_diff = max(0, 0, 255) = 255 → diff=1.0. With a_vis=1.0 and gain=1.0:
    /// alpha_factor=1.0 → fully red. Catches any regression where `min` is used
    /// instead of `max` across channels (min would give 0, missing the B diff).
    #[test]
    fn asymmetric_diff_channel_should_use_max_not_min() {
        let impl_image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        // R=G=0 (no diff), B=1.0 (max diff), A=1.0 (full visibility)
        let similarity = make_similarity(1, 1, Rgba([0.0f32, 0.0, 1.0, 1.0]));
        let overlay_color = Rgba([255, 0, 0, 255]);

        // max_diff_u8=255, diff=1.0, weighted=1.0, alpha_factor=clamp(1.0*1,0,1)=1.0
        let result = render(&impl_image, &similarity, overlay_color, 1.0);
        let px = result.get_pixel(0, 0);
        assert_eq!(
            px[0], 255,
            "R channel must be 255 when B diff channel drives full overlay"
        );
        assert_eq!(px[1], 0, "G channel should be 0");
        assert_eq!(px[2], 0, "B channel should be 0");
        assert_eq!(px[3], 255, "alpha should be preserved");
    }

    /// End-to-end round-trip: uses real `rgba_hybrid_compare` on two synthetic images.
    ///
    /// Two identical all-black 32×32 images, except the reference has a single
    /// different pixel at (0,0) set to (255,0,0,255). After render:
    /// - pixel (0,0) in the output must have notable red overlay (R > impl R)
    /// - pixel (31,31) must be unchanged from impl (SSIM window doesn't reach
    ///   that far from the single different pixel)
    /// - alpha channel at (31,31) must match impl (visibility weight ≠ overlay)
    ///
    /// This is the critical end-to-end test verifying the inversion fix: in
    /// hybrid mode, diff channels are stored as differences (0=match, 1=max diff),
    /// not as similarities.
    #[test]
    fn end_to_end_rgba_hybrid_compare_round_trip() {
        use image_compare::rgba_hybrid_compare;

        // Use a large enough image so SSIM windows don't connect (0,0) to (31,31)
        let mut reference = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 255]));
        let impl_image = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 255]));

        // Reference differs from impl only at (0,0)
        reference.put_pixel(0, 0, Rgba([255, 0, 0, 255]));

        let similarity = rgba_hybrid_compare(&reference, &impl_image)
            .expect("rgba_hybrid_compare should succeed on same-size images");

        let result = render(&impl_image, &similarity, Rgba([255, 0, 0, 255]), 4.0);

        // The differing pixel should have notable red overlay (R > 0)
        let diff_px = result.get_pixel(0, 0);
        assert!(
            diff_px[0] > 0,
            "pixel (0,0) should have red overlay (R={}) when images differ there",
            diff_px[0]
        );

        // The far-corner pixel should be unchanged from impl
        let same_px = result.get_pixel(31, 31);
        let impl_px = impl_image.get_pixel(31, 31);
        assert_eq!(
            same_px, impl_px,
            "pixel (31,31) should be unchanged when images are identical there"
        );
    }
}
