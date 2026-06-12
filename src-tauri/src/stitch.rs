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

#[derive(Clone, Debug)]
pub struct StitchConfig {
    /// Template = this fraction of the effective frame height (min 32 px).
    pub template_ratio: f32,
    /// NCC score below this -> no trusted match.
    pub min_confidence: f32,
    /// Search only within +/- this many rows of the expected position.
    pub inertia_px: u32,
    /// After this many consecutive low-confidence frames, hard-append.
    pub max_lowconf_streak: u32,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            template_ratio: 0.2,
            min_confidence: 0.5,
            inertia_px: 500,
            max_lowconf_streak: 8,
        }
    }
}

pub(crate) fn template_height(eff_h: u32, cfg: &StitchConfig) -> u32 {
    ((eff_h as f32 * cfg.template_ratio) as u32).max(32).min(eff_h.saturating_sub(8))
}

/// NCC search core. `expected_ty` + `inertia` restrict the searched rows.
fn ncc_search(
    prev: &GrayImage,
    new: &GrayImage,
    cfg: &StitchConfig,
    expected_ty: Option<u32>,
    inertia: u32,
) -> Option<(u32, f32)> {
    use imageproc::template_matching::{find_extremes, match_template, MatchTemplateMethod};

    let h = prev.height();
    let t = template_height(h, cfg);
    let template = imageops::crop_imm(prev, 0, h - t, prev.width(), t).to_image();

    // Flat template (blank area) -> NCC is meaningless.
    if !template.pixels().any(|p| p.0[0] >= 8) {
        return None;
    }

    let (lo, hi) = match expected_ty {
        Some(exp) => {
            let lo = exp.saturating_sub(inertia);
            let hi = (exp + inertia).min(h - t);
            (lo, hi)
        }
        None => (0, h - t),
    };
    if lo > hi {
        return None;
    }
    let search = imageops::crop_imm(new, 0, lo, new.width(), hi - lo + t).to_image();

    let result = match_template(&search, &template, MatchTemplateMethod::CrossCorrelationNormalized);
    let ex = find_extremes(&result);
    let conf = ex.max_value;
    if !conf.is_finite() {
        return None;
    }
    Some((lo + ex.max_value_location.1, conf))
}

fn downscale(img: &GrayImage, k: u32) -> GrayImage {
    imageops::resize(
        img,
        (img.width() / k).max(1),
        (img.height() / k).max(1),
        imageops::FilterType::Triangle,
    )
}

/// Windowed coarse-to-fine search core. Frames wider than ~480px are matched
/// on a k-times downscaled copy first (cheap), then refined at full resolution
/// in a +/-(2k+4) row window around the coarse hit — pixel-exact result at a
/// fraction of the cost. Essential for Retina-resolution frames.
///
/// The coarse ty is converted to full-resolution via the scroll-delta domain
/// (delta_small * k ≈ delta_full), which is scale-invariant and avoids
/// template-height rounding errors:
///   full_expected = (h_full - t_full) - ((small_h - t_small) - coarse_ty) * k
pub(crate) fn find_overlap_inner(
    prev: &GrayImage,
    new: &GrayImage,
    cfg: &StitchConfig,
    expected_ty: Option<u32>,
) -> Option<(u32, f32)> {
    let k = (prev.width() / 480).max(1);
    if k == 1 {
        return ncc_search(prev, new, cfg, expected_ty, cfg.inertia_px);
    }
    let small_prev = downscale(prev, k);
    let small_new = downscale(new, k);
    let (coarse_ty, _) = ncc_search(
        &small_prev,
        &small_new,
        cfg,
        expected_ty.map(|e| e / k),
        (cfg.inertia_px / k).max(8),
    )?;
    // Convert coarse ty to full-resolution via scroll-delta domain:
    // delta_small = (small_h - t_small) - coarse_ty  (scale-invariant)
    // full_expected = (h_full - t_full) - delta_small * k
    // This avoids template-height rounding errors that naive coarse_ty*k incurs.
    let small_h = small_prev.height();
    let t_small = template_height(small_h, cfg);
    let h_full = prev.height();
    let t_full = template_height(h_full, cfg);
    let delta_small = (small_h - t_small).saturating_sub(coarse_ty);
    let full_expected = (h_full - t_full).saturating_sub(delta_small * k);
    ncc_search(prev, new, cfg, Some(full_expected), 2 * k + 8)
}

/// Two-attempt search wrapper: first tries the windowed search (inertia-based),
/// then always runs a full-range search when `expected_ty` was Some (i.e., we
/// had an inertia window that might have found a false positive). Returns
/// whichever result has higher confidence.
pub(crate) fn find_overlap(
    prev: &GrayImage,
    new: &GrayImage,
    cfg: &StitchConfig,
    expected_ty: Option<u32>,
) -> Option<(u32, f32)> {
    let windowed = find_overlap_inner(prev, new, cfg, expected_ty);

    // When an inertia window was used, run a full-range search too and take
    // the higher-confidence result. The window can find false positives (small
    // spurious matches) when the actual scroll step changes dramatically;
    // the full-range search finds the globally best match.
    if expected_ty.is_some() {
        let full = find_overlap_inner(prev, new, cfg, None);
        let windowed_conf = windowed.map(|(_, c)| c).unwrap_or(0.0);
        let full_conf = full.map(|(_, c)| c).unwrap_or(0.0);
        if full_conf > windowed_conf {
            return full;
        }
    }

    windowed
}

#[derive(Debug)]
pub enum PushResult {
    First,
    AppendedRows(u32),
    SkippedDuplicate,
    SkippedLowConfidence,
    HardAppended,
}

pub struct Stitcher {
    cfg: StitchConfig,
    canvas: Option<RgbaImage>,
    last: Option<RgbaImage>,
    /// (top, bottom) sticky row counts, detected once from the first scrolled pair.
    sticky: Option<(u32, u32)>,
    /// Sticky footer strip, re-appended once at finish().
    footer: Option<RgbaImage>,
    last_delta: Option<u32>,
    lowconf_streak: u32,
    /// Cached edge image of `last`'s effective region (full-width).
    /// Invariant: only valid after the first append; sticky detection runs
    /// before the first append so any pre-append frames need no cache handling.
    last_eff_edges: Option<GrayImage>,
    /// Horizontal span of columns that change between frames (scrolling content).
    /// Detected once from the first non-duplicate pair alongside sticky detection.
    /// None means use the full width.
    active_cols: Option<(u32, u32)>,
}

impl Stitcher {
    pub fn new(cfg: StitchConfig) -> Self {
        Self {
            cfg,
            canvas: None,
            last: None,
            sticky: None,
            footer: None,
            last_delta: None,
            lowconf_streak: 0,
            last_eff_edges: None,
            active_cols: None,
        }
    }

    pub fn height(&self) -> u32 {
        self.canvas.as_ref().map(|c| c.height()).unwrap_or(0)
            + self.footer.as_ref().map(|f| f.height()).unwrap_or(0)
    }

    fn effective(&self, f: &RgbaImage) -> RgbaImage {
        let (top, bottom) = self.sticky.unwrap_or((0, 0));
        imageops::crop_imm(f, 0, top, f.width(), f.height() - top - bottom).to_image()
    }

    /// Crop an edge image to the active column span for matching.
    /// Full-width edges are stored in the cache; cropping happens at call time.
    fn match_region<'a>(&self, img: &'a GrayImage) -> std::borrow::Cow<'a, GrayImage> {
        match self.active_cols {
            Some((x, w)) => std::borrow::Cow::Owned(
                imageops::crop_imm(img, x, 0, w, img.height()).to_image(),
            ),
            None => std::borrow::Cow::Borrowed(img),
        }
    }

    pub fn push_frame(&mut self, f: &RgbaImage) -> PushResult {
        let Some(last) = self.last.clone() else {
            self.canvas = Some(f.clone());
            self.last = Some(f.clone());
            return PushResult::First;
        };

        if frames_nearly_equal(f, &last) {
            return PushResult::SkippedDuplicate;
        }

        // First scrolled pair: detect sticky edges and active columns, then
        // retroactively trim the footer off the canvas (re-attaches at finish()).
        if self.sticky.is_none() {
            let (top, bottom) = detect_sticky(&last, f);
            self.sticky = Some((top, bottom));
            self.active_cols = detect_active_columns(&last, f);
            if bottom > 0 {
                let canvas = self.canvas.take().unwrap();
                let h = canvas.height();
                self.footer = Some(
                    imageops::crop_imm(&canvas, 0, h - bottom, canvas.width(), bottom).to_image(),
                );
                self.canvas = Some(
                    imageops::crop_imm(&canvas, 0, 0, canvas.width(), h - bottom).to_image(),
                );
            }
        }

        let prev_eff = match self.last_eff_edges.take() {
            Some(cached) => cached,
            None => edges(&self.effective(&last)),
        };
        let new_eff_rgba = self.effective(f);
        let new_eff = edges(&new_eff_rgba);
        let eff_h = prev_eff.height();
        let t = template_height(eff_h, &self.cfg);
        let expected_ty = self.last_delta.map(|d| (eff_h - t).saturating_sub(d));

        match find_overlap(&self.match_region(&prev_eff), &self.match_region(&new_eff), &self.cfg, expected_ty) {
            Some((ty, conf)) if conf >= self.cfg.min_confidence => {
                let delta = (eff_h - t).saturating_sub(ty);
                if delta == 0 {
                    // No movement: restore cache so the next push doesn't recompute.
                    self.last_eff_edges = Some(prev_eff);
                    return PushResult::SkippedDuplicate;
                }
                let new_rows = imageops::crop_imm(
                    &new_eff_rgba,
                    0,
                    eff_h - delta,
                    new_eff_rgba.width(),
                    delta,
                )
                .to_image();
                self.canvas = Some(vstack(self.canvas.as_ref().unwrap(), &new_rows));
                self.last = Some(f.clone());
                self.last_delta = Some(delta);
                self.lowconf_streak = 0;
                // Cache the newly computed edge image for the next push.
                self.last_eff_edges = Some(new_eff);
                PushResult::AppendedRows(delta)
            }
            _ => {
                self.lowconf_streak += 1;
                if self.lowconf_streak >= self.cfg.max_lowconf_streak {
                    // Lost track (scrolled too fast / blank content): append the
                    // whole effective frame; a seam beats losing the content.
                    self.canvas = Some(vstack(self.canvas.as_ref().unwrap(), &new_eff_rgba));
                    self.last = Some(f.clone());
                    self.last_delta = None;
                    self.lowconf_streak = 0;
                    // Cache the newly computed edge image for the next push.
                    self.last_eff_edges = Some(new_eff);
                    PushResult::HardAppended
                } else {
                    // Low confidence skip: restore cache so the next push doesn't recompute.
                    self.last_eff_edges = Some(prev_eff);
                    PushResult::SkippedLowConfidence
                }
            }
        }
    }

    pub fn finish(self) -> RgbaImage {
        let canvas = self.canvas.expect("finish() before any frame");
        match self.footer {
            Some(footer) => vstack(&canvas, &footer),
            None => canvas,
        }
    }
}

/// Columns that changed between two frames (the scrolling content span).
///
/// Samples columns with step 4 and rows with step 4. A pixel differs when
/// any channel diff > 10. A column is "changed" when > 2% of sampled pixels
/// differ. Returns the contiguous span (x_start, width) from the first to the
/// last changed column, snapped to 4-pixel boundaries. Returns None when the
/// span is fewer than 64 columns wide (too narrow to be useful) or when ALL
/// columns changed (no static region → no restriction needed).
pub(crate) fn detect_active_columns(a: &RgbaImage, b: &RgbaImage) -> Option<(u32, u32)> {
    let (w, h) = a.dimensions();
    let sampled_rows: Vec<u32> = (0..h).step_by(4).collect();
    let n_rows = sampled_rows.len() as f32;

    // For each sampled column, count differing pixels.
    let col_changed: Vec<bool> = (0..w)
        .step_by(4)
        .map(|x| {
            let diffs = sampled_rows
                .iter()
                .filter(|&&y| {
                    let pa = a.get_pixel(x, y).0;
                    let pb = b.get_pixel(x, y).0;
                    (0..3).any(|c| (pa[c] as i32 - pb[c] as i32).abs() > 10)
                })
                .count();
            (diffs as f32 / n_rows) > 0.02
        })
        .collect();

    let n_changed = col_changed.iter().filter(|&&c| c).count();
    // All columns changed → full-width scroll, no restriction needed.
    if n_changed == col_changed.len() {
        return None;
    }

    // Find first and last changed sampled-column index.
    let first_idx = col_changed.iter().position(|&c| c)?;
    let last_idx = col_changed.iter().rposition(|&c| c)?;

    // Convert indices back to pixel coordinates (step 4).
    let x_start_raw = (first_idx as u32) * 4;
    let x_end_raw = (last_idx as u32) * 4;

    // Snap x_start down to multiple of 4, extend end up accordingly, clamp.
    let x_start = x_start_raw & !3;
    let x_end = ((x_end_raw + 3) & !3).min(w.saturating_sub(1));
    let width = x_end.saturating_sub(x_start) + 1;
    let width = width.min(w.saturating_sub(x_start));

    if width < 64 {
        return None;
    }

    Some((x_start, width))
}

/// Rows identical between two frames that OTHERWISE differ are fixed UI
/// chrome (sticky headers/footers). Counts consecutive identical rows from
/// the top and bottom; each capped at 30% of frame height.
pub(crate) fn detect_sticky(a: &RgbaImage, b: &RgbaImage) -> (u32, u32) {
    let h = a.height();
    let cap = (h as f32 * 0.3) as u32;
    let row_same = |y: u32| -> bool {
        let (w, mut diff, mut n) = (a.width(), 0u32, 0u32);
        for x in (0..w).step_by(4) {
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            if (0..3).any(|c| (pa[c] as i32 - pb[c] as i32).abs() > 10) {
                diff += 1;
            }
            n += 1;
        }
        (diff as f32 / n as f32) < 0.02
    };
    let mut top = 0;
    while top < cap && row_same(top) {
        top += 1;
    }
    let mut bottom = 0;
    while bottom < cap && row_same(h - 1 - bottom) {
        bottom += 1;
    }
    (top, bottom)
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

    #[test]
    fn find_overlap_locates_exact_scroll() {
        let src = make_source(300, 900, 7);
        let prev = edges(&viewport(&src, 0, 300));
        let new = edges(&viewport(&src, 120, 300));
        let cfg = StitchConfig::default();
        // template = bottom strip of prev; in `new` it sits 120 rows higher
        let (ty, conf) = find_overlap(&prev, &new, &cfg, None).expect("match");
        let t = template_height(prev.height(), &cfg);
        assert_eq!((prev.height() - t) - ty, 120);
        assert!(conf > 0.8);
    }

    #[test]
    fn find_overlap_rejects_flat_template() {
        let flat = RgbaImage::from_pixel(300, 300, Rgba([255, 255, 255, 255]));
        let e = edges(&flat);
        let cfg = StitchConfig::default();
        assert!(find_overlap(&e, &e, &cfg, None).is_none());
    }

    #[test]
    fn inertia_window_excludes_far_matches() {
        // This test proves the WINDOW restricts the search. The windowed core
        // (`find_overlap_inner`) is called directly; `find_overlap` (the wrapper)
        // adds a full-range fallback on top and would recover the true position.
        let src = make_source(300, 2000, 9);
        let prev = edges(&viewport(&src, 0, 300));
        let new = edges(&viewport(&src, 40, 300));
        let cfg = StitchConfig { inertia_px: 20, ..StitchConfig::default() };
        let t = template_height(prev.height(), &cfg);
        // true ty for a 40px scroll:
        let true_ty = (prev.height() - t) - 40;
        // expectation centered 200px away from truth with a ±20px window
        let far_expected = true_ty.saturating_sub(200);
        let found = find_overlap_inner(&prev, &new, &cfg, Some(far_expected));
        // the true position is outside the window, so either nothing is found
        // or whatever is found is NOT the true offset with high confidence
        if let Some((ty, conf)) = found {
            assert!(ty != true_ty || conf < cfg.min_confidence);
        }
    }

    fn push_all(stitcher: &mut Stitcher, frames: &[RgbaImage]) -> Vec<PushResult> {
        frames.iter().map(|f| stitcher.push_frame(f)).collect()
    }

    #[test]
    fn stitches_scrolled_frames_back_into_source() {
        let src = make_source(300, 1200, 11);
        let view = 300;
        let offsets = [0u32, 130, 260, 430, 600];
        let frames: Vec<_> = offsets.iter().map(|o| viewport(&src, *o, view)).collect();
        let mut s = Stitcher::new(StitchConfig::default());
        let results = push_all(&mut s, &frames);
        assert!(matches!(results[0], PushResult::First));
        for r in &results[1..] {
            assert!(matches!(r, PushResult::AppendedRows(_)), "got {r:?}");
        }
        let out = s.finish();
        let expected = viewport(&src, 0, 600 + view);
        assert_eq!(out.dimensions(), expected.dimensions());
        assert_eq!(out, expected);
    }

    #[test]
    fn duplicate_frames_are_skipped() {
        let src = make_source(300, 600, 13);
        let f = viewport(&src, 0, 300);
        let mut s = Stitcher::new(StitchConfig::default());
        s.push_frame(&f);
        assert!(matches!(s.push_frame(&f), PushResult::SkippedDuplicate));
        assert_eq!(s.finish().dimensions(), (300, 300));
    }

    #[test]
    fn sticky_header_and_footer_appear_once() {
        let content = make_source(300, 1500, 17);
        let header = make_source(300, 40, 99);
        let footer = make_source(300, 30, 101);
        let view = 300;
        let body_h = view - 40 - 30;
        let frame = |off: u32| {
            let body = viewport(&content, off, body_h);
            vstack(&vstack(&header, &body), &footer)
        };
        let offsets = [0u32, 100, 200, 300];
        let frames: Vec<_> = offsets.iter().map(|o| frame(*o)).collect();
        let mut s = Stitcher::new(StitchConfig::default());
        push_all(&mut s, &frames);
        let out = s.finish();
        // header once + full scrolled body + footer once
        let expected = vstack(&vstack(&header, &viewport(&content, 0, 300 + body_h)), &footer);
        assert_eq!(out.dimensions(), expected.dimensions());
        assert_eq!(out, expected);
    }

    #[test]
    fn lowconf_streak_hard_appends() {
        let src = make_source(300, 600, 19);
        let mut s = Stitcher::new(StitchConfig { max_lowconf_streak: 3, ..StitchConfig::default() });
        s.push_frame(&viewport(&src, 0, 300));
        let flat = RgbaImage::from_pixel(300, 300, Rgba([250, 250, 250, 255]));
        assert!(matches!(s.push_frame(&flat), PushResult::SkippedLowConfidence));
        // slightly different flats so the duplicate check doesn't trip first
        let flat2 = RgbaImage::from_pixel(300, 300, Rgba([244, 244, 244, 255]));
        let flat3 = RgbaImage::from_pixel(300, 300, Rgba([238, 238, 238, 255]));
        assert!(matches!(s.push_frame(&flat2), PushResult::SkippedLowConfidence));
        assert!(matches!(s.push_frame(&flat3), PushResult::HardAppended));
        assert_eq!(s.finish().height(), 600);
    }

    #[test]
    fn single_frame_finish_returns_it() {
        let src = make_source(300, 400, 23);
        let f = viewport(&src, 0, 300);
        let mut s = Stitcher::new(StitchConfig::default());
        s.push_frame(&f);
        assert_eq!(s.finish(), f);
    }

    #[test]
    fn wide_retina_frames_stitch_exactly() {
        // width 1024 -> downscale factor k=2 -> coarse+refine path
        let src = make_source(1024, 1600, 31);
        let view = 400;
        let offsets = [0u32, 150, 300, 450];
        let frames: Vec<_> = offsets.iter().map(|o| viewport(&src, *o, view)).collect();
        let mut s = Stitcher::new(StitchConfig::default());
        for f in &frames {
            s.push_frame(f);
        }
        let out = s.finish();
        let expected = viewport(&src, 0, 450 + view);
        assert_eq!(out.dimensions(), expected.dimensions());
        assert_eq!(out, expected);
    }

    #[test]
    fn detect_active_columns_finds_scrolling_span() {
        let content = make_source(300, 900, 41);
        let sidebar = make_source(200, 300, 43); // static left pane
        let frame = |off: u32| {
            let body = viewport(&content, off, 300);
            // hstack: sidebar (static) | body (scrolls)
            let mut f = RgbaImage::new(500, 300);
            image::imageops::replace(&mut f, &sidebar, 0, 0);
            image::imageops::replace(&mut f, &body, 200, 0);
            f
        };
        let (a, b) = (frame(0), frame(120));
        let span = detect_active_columns(&a, &b).expect("span");
        // span must start at/near the body boundary and cover the body
        assert!(span.0 >= 196 && span.0 <= 204, "x_start = {}", span.0);
        assert!(span.0 + span.1 >= 496, "span end = {}", span.0 + span.1);
    }

    #[test]
    fn detect_active_columns_none_when_everything_scrolls() {
        let src = make_source(300, 900, 47);
        let a = viewport(&src, 0, 300);
        let b = viewport(&src, 120, 300);
        assert!(detect_active_columns(&a, &b).is_none());
    }

    #[test]
    fn bursty_scroll_speed_recovers_via_full_search() {
        // Scroll step jumps from 60 to 700 rows — far beyond the inertia
        // window around the expected position — while plenty of overlap
        // remains (view 1200, template ~240). Matching must fall back to a
        // full-range search and append exactly.
        let src = make_source(300, 3000, 61);
        let view = 1200;
        let offsets = [0u32, 60, 760, 820];
        let mut s = Stitcher::new(StitchConfig::default());
        let results: Vec<_> = offsets
            .iter()
            .map(|o| s.push_frame(&viewport(&src, *o, view)))
            .collect();
        assert!(matches!(results[0], PushResult::First));
        let expect = [60u32, 700, 60];
        for (i, r) in results[1..].iter().enumerate() {
            match r {
                PushResult::AppendedRows(d) => assert_eq!(*d, expect[i], "frame {}", i + 1),
                other => panic!("frame {} got {:?}", i + 1, other),
            }
        }
        let out = s.finish();
        assert_eq!(out, viewport(&src, 0, 820 + view));
    }

    #[test]
    fn static_sidebar_does_not_poison_scroll_matching() {
        let content = make_source(400, 1200, 53);
        let sidebar = make_source(300, 300, 59);
        let frame = |off: u32| {
            let body = viewport(&content, off, 300);
            let mut f = RgbaImage::new(700, 300);
            image::imageops::replace(&mut f, &sidebar, 0, 0);
            image::imageops::replace(&mut f, &body, 300, 0);
            f
        };
        let offsets = [0u32, 110, 220, 330];
        let mut s = Stitcher::new(StitchConfig::default());
        let results: Vec<_> = offsets.iter().map(|o| s.push_frame(&frame(*o))).collect();
        assert!(matches!(results[0], PushResult::First));
        // Every scrolled frame must be APPENDED with the TRUE delta — the
        // static sidebar must not cause SkippedDuplicate or wrong deltas.
        for (i, r) in results[1..].iter().enumerate() {
            match r {
                PushResult::AppendedRows(d) => assert_eq!(*d, 110, "frame {} delta", i + 1),
                other => panic!("frame {} got {:?}", i + 1, other),
            }
        }
        let out = s.finish();
        // Right side of the output must equal the scrolled content exactly.
        let out_body = image::imageops::crop_imm(&out, 300, 0, 400, out.height()).to_image();
        let expected_body = viewport(&content, 0, 330 + 300);
        assert_eq!(out_body, expected_body);
    }
}
