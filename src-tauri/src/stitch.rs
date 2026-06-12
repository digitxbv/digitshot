//! Pure frame-stitching engine for scrolling capture. Operates entirely in
//! physical pixels on `RgbaImage` frames of identical dimensions. No I/O.

use image::{imageops, DynamicImage, GrayImage, RgbaImage};

/// Grayscale -> Sobel gradient magnitude, clamped to u8. Matching in the
/// edge domain makes NCC robust on low-contrast (mostly white) pages.
pub(crate) fn edges(rgba: &RgbaImage) -> GrayImage {
    let gray = DynamicImage::ImageRgba8(rgba.clone()).into_luma8();
    let grad = imageproc::gradients::sobel_gradients(&gray);
    GrayImage::from_fn(grad.width(), grad.height(), |x, y| {
        image::Luma([(grad.get_pixel(x, y).0[0] >> 2).min(255) as u8])
    })
}

/// Cheap sampled comparison: mean absolute channel difference over a sparse
/// grid. Used to drop frames where the user hasn't scrolled yet.
pub(crate) fn frames_nearly_equal(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.dimensions() != b.dimensions() {
        return false;
    }
    let (w, h) = a.dimensions();
    let mut total: u64 = 0;
    let mut count: u64 = 0;
    let step = 8;
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            for c in 0..3 {
                total += (pa[c] as i32 - pb[c] as i32).unsigned_abs() as u64;
            }
            count += 3;
        }
    }
    (total as f64 / count as f64) < 2.0
}

pub(crate) fn vstack(top: &RgbaImage, bottom: &RgbaImage) -> RgbaImage {
    assert_eq!(top.width(), bottom.width());
    let mut out = RgbaImage::new(top.width(), top.height() + bottom.height());
    imageops::replace(&mut out, top, 0, 0);
    imageops::replace(&mut out, bottom, 0, top.height() as i64);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// Deterministic pseudo-random tall source image: horizontal 4px bands of
    /// LCG-derived colors — high texture, no repeating pattern.
    pub(crate) fn make_source(w: u32, h: u32, seed: u64) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut band = [0u8; 3];
        for y in 0..h {
            if y % 4 == 0 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                band = [(state >> 16) as u8, (state >> 24) as u8, (state >> 32) as u8];
            }
            for x in 0..w {
                // slight horizontal variation so columns are not constant
                let v = band[0].wrapping_add((x % 7) as u8 * 3);
                img.put_pixel(x, y, Rgba([v, band[1], band[2], 255]));
            }
        }
        img
    }

    pub(crate) fn viewport(src: &RgbaImage, offset: u32, view_h: u32) -> RgbaImage {
        image::imageops::crop_imm(src, 0, offset, src.width(), view_h).to_image()
    }

    #[test]
    fn nearly_equal_detects_identical_and_different() {
        let src = make_source(200, 400, 1);
        let a = viewport(&src, 0, 100);
        let b = viewport(&src, 0, 100);
        let c = viewport(&src, 50, 100);
        assert!(frames_nearly_equal(&a, &b));
        assert!(!frames_nearly_equal(&a, &c));
    }

    #[test]
    fn vstack_concatenates() {
        let src = make_source(64, 64, 2);
        let top = viewport(&src, 0, 24);
        let bottom = viewport(&src, 24, 40);
        let joined = vstack(&top, &bottom);
        assert_eq!(joined.dimensions(), (64, 64));
        assert_eq!(joined, src);
    }

    #[test]
    fn edges_of_flat_image_are_flat() {
        let flat = RgbaImage::from_pixel(100, 100, Rgba([128, 128, 128, 255]));
        let e = edges(&flat);
        assert!(e.pixels().all(|p| p.0[0] < 8));
        let textured = make_source(100, 100, 3);
        let et = edges(&textured);
        assert!(et.pixels().any(|p| p.0[0] >= 8));
    }
}
