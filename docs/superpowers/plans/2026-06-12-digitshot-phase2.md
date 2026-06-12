# DigitShot Phase 2 Implementation Plan — Scrolling Capture + Queue Persistence

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Manual-scroll scrolling capture (region select → user scrolls → stitched tall PNG into the existing pipeline) plus thumbnail-queue persistence across restarts and tall-image editor fit.

**Architecture:** Rust captures the selected region at ~3fps via `xcap` (logical coords in, physical-resolution frames out) on a session thread feeding a pure `Stitcher` (grayscale→Sobel-edge NCC template matching with an inertia window, dynamic sticky-edge trim). A single extra `selector` NSPanel window morphs from fullscreen drag-select veil into a floating Done/Cancel control panel. Queue persistence = overlay `localStorage` + a `filter_existing` command.

**Tech Stack:** existing phase-1 stack + `xcap 0.9`, `imageproc 0.27` (with `image 0.25`, already present), CoreGraphics FFI for the Screen Recording preflight.

**Spec:** `docs/superpowers/specs/2026-06-12-digitshot-phase2-design.md`

**Conventions:** repo root `/Users/patrickgerrits/Development/DigitShot`, branch `feature/phase-2`. Commit per task, end commit messages with the `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` trailer (blank line before). NEVER run `npm run tauri dev` (orchestrator handles live runs); verify with `cargo check`/`cargo test`/`npm run test`/`npm run build`. Commit with explicit file paths. A dev server may be running on port 1420 — never start another.

**Verified API facts (researched 2026-06-12, do not re-derive):**
- `xcap 0.9.6`: `Monitor::all() -> XCapResult<Vec<Monitor>>`; `m.is_primary() -> XCapResult<bool>`; `m.width()/height() -> XCapResult<u32>` (LOGICAL points); `m.scale_factor() -> XCapResult<f32>`; `m.capture_region(x, y, w, h) -> XCapResult<RgbaImage>` takes LOGICAL points relative to the monitor origin and returns a PHYSICAL-resolution image (400×300 logical on 2× Retina → 800×600 buffer). Without the Screen Recording grant it silently returns wallpaper-only images — that's why the FFI preflight gates every session.
- `imageproc 0.27`: `template_matching::{match_template, MatchTemplateMethod::CrossCorrelationNormalized, find_extremes}`; `match_template(&GrayImage, &GrayImage, method) -> Image<Luma<f32>>`; `Extremes { max_value, max_value_location: (u32, u32), .. }`. `gradients::sobel_gradients(&GrayImage) -> Image<Luma<u16>>`.
- `image 0.25`: `imageops::crop_imm(&img, x, y, w, h).to_image()` for owned crops; `imageops::replace(&mut dst, &src, x_i64, y_i64)` for blits; `img.save(path)` infers PNG from extension; `DynamicImage::ImageRgba8(rgba).into_luma8()` for grayscale.

---

### Task 1: Dependencies, Screen Recording preflight, selector window scaffold

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `vite.config.ts`, `src-tauri/src/lib.rs`
- Create: `src-tauri/src/permissions.rs`, `selector.html`, `src/selector/main.ts`, `src/selector/SelectorApp.vue` (placeholder)

- [ ] **Step 1: Add Rust deps to `src-tauri/Cargo.toml`:**

```toml
xcap = "0.9"
imageproc = "0.27"
```

- [ ] **Step 2: Create `src-tauri/src/permissions.rs`:**

```rust
//! Screen Recording TCC preflight. xcap silently returns wallpaper-only
//! frames without the grant, so every scroll session is gated on this.

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
pub fn has_screen_recording() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
pub fn request_screen_recording() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(not(target_os = "macos"))]
pub fn has_screen_recording() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording() -> bool {
    true
}
```

Add `mod permissions;` to `lib.rs`.

- [ ] **Step 3: Selector window in `src-tauri/tauri.conf.json`** — append to `app.windows`:

```json
{
  "label": "selector",
  "url": "selector.html",
  "title": "DigitShot Selector",
  "visible": false,
  "transparent": true,
  "decorations": false,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "resizable": false,
  "shadow": false,
  "acceptFirstMouse": true,
  "width": 400,
  "height": 300
}
```

Add `"selector"` to the `windows` array in `src-tauri/capabilities/default.json`.

- [ ] **Step 4: Frontend entry.** `selector.html` (same shape as `editor.html`, script `/src/selector/main.ts`, title "DigitShot Selector"). `src/selector/main.ts`:

```ts
import { createApp } from "vue";
import SelectorApp from "./SelectorApp.vue";

createApp(SelectorApp).mount("#app");
```

Placeholder `src/selector/SelectorApp.vue`: `<template><div></div></template>`. Add `selector: resolve(__dirname, "selector.html")` to `build.rollupOptions.input` in `vite.config.ts`.

- [ ] **Step 5: Convert selector to NSPanel in `lib.rs`.** Extend the existing `tauri_panel!` block with a second panel type (the macro from the nspanel v2.1 branch takes the panel name at top level; mirror how `OverlayPanel` is declared):

```rust
#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    SelectorPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    }
}
```

(If the macro only accepts one panel per invocation, write a second `tauri_panel! { ... }` block.) In `setup`, after the overlay panel block:

```rust
#[cfg(target_os = "macos")]
{
    let window = app.get_webview_window("selector").unwrap();
    let panel = window.to_panel::<SelectorPanel>().unwrap();
    panel.set_level(PanelLevel::ScreenSaver.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    panel.set_hides_on_deactivate(false);
}
```

`can_become_key_window: true` is required — the selector page must receive Esc keydowns. `ScreenSaver` level so the veil sits above everything during selection.

- [ ] **Step 6: Verify + commit**

```bash
cd src-tauri && cargo check && cargo test && cd ..
npm run test && npm run build
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/capabilities/default.json src-tauri/src/permissions.rs src-tauri/src/lib.rs vite.config.ts selector.html src/selector/main.ts src/selector/SelectorApp.vue
git commit -m "feat: phase 2 scaffold — xcap/imageproc deps, screen-recording preflight, selector panel"
```

---

### Task 2: Stitcher part 1 — edge transform, frame comparison, blit helpers (TDD)

**Files:**
- Create: `src-tauri/src/stitch.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod stitch;`)

All stitching code is pure: no Tauri types, no I/O. Tests live in `#[cfg(test)] mod tests` inside `stitch.rs` and build synthetic images with a seeded LCG (deterministic, no `rand` dep).

- [ ] **Step 1: Write the failing tests** (bottom of the new `stitch.rs`; the file starts with just the test helpers + `todo!()`-free signatures from Step 2 — write tests first, watch them fail to compile, then implement):

```rust
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
```

- [ ] **Step 2: Implement the helpers** (top of `stitch.rs`):

```rust
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
```

Add `mod stitch;` to `lib.rs`.

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test stitch
```

Expected: 3 tests pass (plus phase-1 capture tests still green via plain `cargo test`).

```bash
git add src-tauri/src/stitch.rs src-tauri/src/lib.rs
git commit -m "feat: stitch helpers — edge transform, frame comparison, vstack (TDD)"
```

---

### Task 3: Stitcher part 2 — overlap search with inertia window (TDD)

**Files:**
- Modify: `src-tauri/src/stitch.rs`

- [ ] **Step 1: Failing tests** (append to the tests module):

```rust
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
        let src = make_source(300, 2000, 9);
        let prev = edges(&viewport(&src, 0, 300));
        let new = edges(&viewport(&src, 40, 300));
        let cfg = StitchConfig { inertia_px: 20, ..StitchConfig::default() };
        let t = template_height(prev.height(), &cfg);
        // true ty for a 40px scroll:
        let true_ty = (prev.height() - t) - 40;
        // expectation centered 200px away from truth with a ±20px window
        let far_expected = true_ty.saturating_sub(200);
        let found = find_overlap(&prev, &new, &cfg, Some(far_expected));
        // the true position is outside the window, so either nothing is found
        // or whatever is found is NOT the true offset with high confidence
        if let Some((ty, conf)) = found {
            assert!(ty != true_ty || conf < cfg.min_confidence);
        }
    }
```

- [ ] **Step 2: Implement** (above the tests module):

```rust
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
            max_lowconf_streak: 3,
        }
    }
}

pub(crate) fn template_height(eff_h: u32, cfg: &StitchConfig) -> u32 {
    ((eff_h as f32 * cfg.template_ratio) as u32).max(32).min(eff_h.saturating_sub(8))
}

/// Searches for `prev`'s bottom template strip inside `new` (both edge-domain,
/// same dimensions). Returns (ty, confidence): ty = row in `new` where the
/// template's top edge matches. Scroll distance = (h - template_h) - ty.
/// `expected_ty` (from the previous scroll delta) restricts the search to an
/// inertia window, defeating repeated-content false matches.
pub(crate) fn find_overlap(
    prev: &GrayImage,
    new: &GrayImage,
    cfg: &StitchConfig,
    expected_ty: Option<u32>,
) -> Option<(u32, f32)> {
    use imageproc::template_matching::{find_extremes, match_template, MatchTemplateMethod};

    let h = prev.height();
    let t = template_height(h, cfg);
    let template = imageops::crop_imm(prev, 0, h - t, prev.width(), t).to_image();

    // Flat template (blank area) -> NCC is meaningless.
    if !template.pixels().any(|p| p.0[0] >= 8) {
        return None;
    }

    // Restrict search rows to the inertia window around the expectation.
    let (lo, hi) = match expected_ty {
        Some(exp) => {
            let lo = exp.saturating_sub(cfg.inertia_px);
            let hi = (exp + cfg.inertia_px).min(h - t);
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
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test stitch
git add src-tauri/src/stitch.rs
git commit -m "feat: stitch overlap search — edge-domain NCC with inertia window (TDD)"
```

---

### Task 4: Stitcher part 3 — state machine with sticky-edge handling (TDD)

**Files:**
- Modify: `src-tauri/src/stitch.rs`

- [ ] **Step 1: Failing tests** (append):

```rust
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
        let mut s = Stitcher::new(StitchConfig::default());
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
```

- [ ] **Step 2: Implement the state machine:**

```rust
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

    pub fn push_frame(&mut self, f: &RgbaImage) -> PushResult {
        let Some(last) = self.last.clone() else {
            self.canvas = Some(f.clone());
            self.last = Some(f.clone());
            return PushResult::First;
        };

        if frames_nearly_equal(f, &last) {
            return PushResult::SkippedDuplicate;
        }

        // First scrolled pair: detect sticky edges and retroactively trim
        // the footer off the canvas (it re-attaches once at finish()).
        if self.sticky.is_none() {
            let (top, bottom) = detect_sticky(&last, f);
            self.sticky = Some((top, bottom));
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

        let prev_eff = edges(&self.effective(&last));
        let new_eff_rgba = self.effective(f);
        let new_eff = edges(&new_eff_rgba);
        let eff_h = prev_eff.height();
        let t = template_height(eff_h, &self.cfg);
        let expected_ty = self.last_delta.map(|d| (eff_h - t).saturating_sub(d));

        match find_overlap(&prev_eff, &new_eff, &self.cfg, expected_ty) {
            Some((ty, conf)) if conf >= self.cfg.min_confidence => {
                let delta = (eff_h - t).saturating_sub(ty);
                if delta == 0 {
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
                    PushResult::HardAppended
                } else {
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
```

- [ ] **Step 3: Run + commit.** `cargo test stitch` — all stitch tests green (the exact-equality assertions are the point: stitching must be pixel-perfect on synthetic frames). If `stitches_scrolled_frames_back_into_source` is flaky on the 430→600 gap (170 px > template overlap), the offsets guarantee overlap ≥ 130 rows with view 300 and template 60 — they are chosen to pass; investigate rather than loosen assertions.

```bash
git add src-tauri/src/stitch.rs
git commit -m "feat: stitcher state machine — sticky edges, dup/low-confidence handling (TDD)"
```

---

### Task 5: Scroll session, commands, hotkey, tray

**Files:**
- Create: `src-tauri/src/scrolling.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/scrolling.rs`:**

```rust
//! Scrolling-capture session: a thread captures the selected region at ~3fps
//! via xcap and feeds the Stitcher; stop() finishes, saves, and re-enters the
//! normal capture pipeline. One session at a time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::stitch::{StitchConfig, Stitcher};

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    frames: u32,
    height: u32,
}

pub struct Session {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Stitcher>,
}

#[derive(Default)]
pub struct ScrollState(pub Mutex<Option<Session>>);

const FRAME_INTERVAL_MS: u64 = 350;

pub fn start(app: AppHandle, region: Region) -> Result<Session, String> {
    let monitor = xcap::Monitor::all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or("no primary monitor")?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let handle = std::thread::spawn(move || {
        let mut stitcher = Stitcher::new(StitchConfig::default());
        let mut frames: u32 = 0;
        while !stop_t.load(Ordering::Relaxed) {
            // Sleep BEFORE the first frame: the selector veil has just been
            // shrunk into the control panel and the window move must settle,
            // or the veil gets baked into frame 1.
            std::thread::sleep(std::time::Duration::from_millis(FRAME_INTERVAL_MS));
            if stop_t.load(Ordering::Relaxed) {
                break;
            }
            match monitor.capture_region(region.x, region.y, region.width, region.height) {
                Ok(frame) => {
                    stitcher.push_frame(&frame);
                    frames += 1;
                    let _ = app.emit(
                        "scroll-progress",
                        ProgressPayload { frames, height: stitcher.height() },
                    );
                }
                Err(e) => {
                    eprintln!("scroll frame capture failed: {e}");
                    let _ = app.emit("scroll-capture-error", e.to_string());
                    break;
                }
            }
        }
        stitcher
    });

    // NOTE: if xcap::Monitor is not Send (compile error on the spawn), capture
    // the region/monitor-id instead and re-resolve Monitor::all() inside the
    // thread — adapt minimally.

    Ok(Session { stop, handle })
}

/// Stops the loop and returns the stitched image (None if no frame was captured).
pub fn stop(session: Session) -> Option<image::RgbaImage> {
    session.stop.store(true, Ordering::Relaxed);
    let stitcher = session.handle.join().ok()?;
    if stitcher.height() == 0 {
        return None;
    }
    Some(stitcher.finish())
}

pub fn cancel(session: Session) {
    session.stop.store(true, Ordering::Relaxed);
    let _ = session.handle.join();
}
```

- [ ] **Step 2: Commands + wiring in `lib.rs`.** Add `mod scrolling;`, `.manage(scrolling::ScrollState::default())` on the builder, and these commands (register all in `generate_handler!`):

```rust
/// Entry point for tray/hotkey/selector. Gates on the Screen Recording grant.
fn begin_scroll_capture(app: &AppHandle) {
    if !permissions::has_screen_recording() {
        permissions::request_screen_recording(); // triggers the system dialog once
        use tauri_plugin_dialog::DialogExt;
        app.dialog()
            .message(
                "Scrolling capture needs Screen Recording permission.\n\n\
                 Grant it in System Settings → Privacy & Security → Screen Recording, \
                 then relaunch DigitShot.",
            )
            .title("Screen Recording required")
            .show(|_| {});
        return;
    }
    // Size the selector to the primary monitor and show it.
    if let Some(win) = app.get_webview_window("selector") {
        let _ = win.emit("selector-reset", ());
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let scale = monitor.scale_factor();
            let size = monitor.size();
            let logical_w = size.width as f64 / scale;
            let logical_h = size.height as f64 / scale;
            let _ = win.set_size(tauri::LogicalSize::new(logical_w, logical_h));
            let _ = win.set_position(tauri::LogicalPosition::new(0.0, 0.0));
        }
        #[cfg(target_os = "macos")]
        {
            use tauri_nspanel::ManagerExt;
            if let Ok(panel) = app.get_webview_panel("selector") {
                panel.show_and_make_key(); // selector needs Esc keydowns
            }
        }
    }
}

#[tauri::command]
fn scroll_capture_begin(app: AppHandle) {
    begin_scroll_capture(&app);
}

#[tauri::command]
fn scroll_capture_start(
    app: AppHandle,
    state: tauri::State<scrolling::ScrollState>,
    region: scrolling::Region,
) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();
    if guard.is_some() {
        return Err("session already running".into());
    }
    *guard = Some(scrolling::start(app, region)?);
    Ok(())
}

#[tauri::command]
fn scroll_capture_stop(
    app: AppHandle,
    state: tauri::State<scrolling::ScrollState>,
) -> Result<(), String> {
    let session = state.0.lock().unwrap().take().ok_or("no session")?;
    hide_selector(&app);
    if let Some(stitched) = scrolling::stop(session) {
        let dir = capture::captures_dir(&home_dir());
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(capture::capture_filename(&chrono::Local::now()));
        stitched.save(&path).map_err(|e| e.to_string())?;
        if let Err(e) = clipboard::copy_png_file(&path) {
            eprintln!("clipboard copy failed: {e}");
        }
        let _ = app.emit(
            "capture-taken",
            CapturePayload { path: path.to_string_lossy().into_owned() },
        );
    }
    Ok(())
}

#[tauri::command]
fn scroll_capture_cancel(app: AppHandle, state: tauri::State<scrolling::ScrollState>) {
    if let Some(session) = state.0.lock().unwrap().take() {
        scrolling::cancel(session);
    }
    hide_selector(&app);
}

/// Shrink the selector window into the control panel, placed outside the region.
#[tauri::command]
fn position_scroll_panel(app: AppHandle, region: scrolling::Region) -> Result<(), String> {
    let win = app.get_webview_window("selector").ok_or("no selector window")?;
    let monitor = win
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("no primary monitor")?;
    let scale = monitor.scale_factor();
    let mon_h = monitor.size().height as f64 / scale;
    let mon_w = monitor.size().width as f64 / scale;
    const PANEL_W: f64 = 380.0;
    const PANEL_H: f64 = 52.0;
    let mut y = (region.y + region.height) as f64 + 12.0;
    if y + PANEL_H > mon_h {
        y = (region.y as f64 - PANEL_H - 12.0).max(8.0);
    }
    let x = ((region.x as f64) + (region.width as f64) / 2.0 - PANEL_W / 2.0)
        .clamp(8.0, mon_w - PANEL_W - 8.0);
    win.set_size(tauri::LogicalSize::new(PANEL_W, PANEL_H)).map_err(|e| e.to_string())?;
    win.set_position(tauri::LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    Ok(())
}

fn hide_selector(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel("selector") {
            panel.hide();
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("selector") {
        let _ = w.hide();
    }
}
```

NOTE: `show_and_make_key` — verify the exact method name on the nspanel v2.1 panel handle (phase 1 used `show()`; the basic example shows `show_and_make_key()` exists). If absent, use `show()` followed by `win.set_focus()`.

- [ ] **Step 3: Second hotkey.** In the global-shortcut setup block, register a scroll shortcut alongside the capture one and dispatch on which fired:

```rust
let capture_shortcut = if cfg!(debug_assertions) {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Digit1)
} else {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Digit2)
};
// Scrolling capture: release Cmd+Shift+6, dev Cmd+Shift+0 (3/4/5 are macOS's).
let scroll_shortcut = if cfg!(debug_assertions) {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Digit0)
} else {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Digit6)
};
app.handle().plugin(
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, sc, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if sc == &capture_shortcut {
                trigger_capture(app);
            } else if sc == &scroll_shortcut {
                begin_scroll_capture(app);
            }
        })
        .build(),
)?;
if let Err(e) = app.global_shortcut().register(capture_shortcut) {
    eprintln!("hotkey registration failed: {e}");
}
if let Err(e) = app.global_shortcut().register(scroll_shortcut) {
    eprintln!("scroll hotkey registration failed: {e}");
}
```

- [ ] **Step 4: Tray item.** Add `let scroll_item = MenuItem::with_id(app, "scroll", "Scrolling Capture", true, None::<&str>)?;` after `capture_item`, include it in `Menu::with_items(app, &[&capture_item, &scroll_item, &folder_item, &quit_item])?`, and handle `"scroll" => begin_scroll_capture(app),` in `on_menu_event`.

- [ ] **Step 5: Verify + commit**

```bash
cd src-tauri && cargo check && cargo test
git add src-tauri/src/scrolling.rs src-tauri/src/lib.rs
git commit -m "feat: scrolling-capture session, commands, hotkey, tray item"
```

---

### Task 6: Selector frontend — drag-select veil + control panel

**Files:**
- Replace: `src/selector/SelectorApp.vue`

- [ ] **Step 1: Implement `SelectorApp.vue`.** Two modes in one component, plain DOM (no Konva):

```vue
<template>
  <!-- SELECT MODE: fullscreen veil + drag rectangle -->
  <div
    v-if="mode === 'select'"
    class="veil"
    @mousedown="onDown"
    @mousemove="onMove"
    @mouseup="onUp"
  >
    <template v-if="rect">
      <div class="shade" :style="{ left: 0, top: 0, width: '100%', height: rect.y + 'px' }" />
      <div class="shade" :style="{ left: 0, top: rect.y + rect.height + 'px', width: '100%', bottom: 0 }" />
      <div class="shade" :style="{ left: 0, top: rect.y + 'px', width: rect.x + 'px', height: rect.height + 'px' }" />
      <div class="shade" :style="{ left: rect.x + rect.width + 'px', top: rect.y + 'px', right: 0, height: rect.height + 'px' }" />
      <div class="marquee" :style="{ left: rect.x + 'px', top: rect.y + 'px', width: rect.width + 'px', height: rect.height + 'px' }">
        <span class="size-label">{{ rect.width }} × {{ rect.height }}</span>
      </div>
    </template>
    <div v-else class="shade full" />
    <div class="instructions">Drag to select the scroll area — Esc to cancel</div>
  </div>

  <!-- PANEL MODE: floating control bar during capture -->
  <div v-else class="panel">
    <span class="rec-dot" />
    <span class="panel-text">Scroll the content… {{ frames }} frames · {{ heightPx }}px</span>
    <button class="done" @click="done">Done</button>
    <button @click="cancel">Cancel</button>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Mode = "select" | "panel";
interface Rect { x: number; y: number; width: number; height: number }

const mode = ref<Mode>("select");
const rect = ref<Rect | null>(null);
const frames = ref(0);
const heightPx = ref(0);

let dragging = false;
let startX = 0;
let startY = 0;

function onDown(e: MouseEvent) {
  dragging = true;
  startX = e.clientX;
  startY = e.clientY;
  rect.value = { x: startX, y: startY, width: 0, height: 0 };
}

function onMove(e: MouseEvent) {
  if (!dragging) return;
  rect.value = {
    x: Math.min(startX, e.clientX),
    y: Math.min(startY, e.clientY),
    width: Math.abs(e.clientX - startX),
    height: Math.abs(e.clientY - startY),
  };
}

async function onUp() {
  if (!dragging) return;
  dragging = false;
  const r = rect.value;
  if (!r || r.width < 24 || r.height < 24) {
    rect.value = null;
    return;
  }
  const region = {
    x: Math.round(r.x),
    y: Math.round(r.y),
    width: Math.round(r.width),
    height: Math.round(r.height),
  };
  frames.value = 0;
  heightPx.value = 0;
  // ORDER MATTERS: shrink the veil into the panel BEFORE starting capture,
  // or the fullscreen veil is baked into the first frames.
  mode.value = "panel";
  await invoke("position_scroll_panel", { region });
  await invoke("scroll_capture_start", { region });
}

function done() {
  invoke("scroll_capture_stop");
}

function cancel() {
  invoke("scroll_capture_cancel");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (mode.value === "panel") cancel();
    else invoke("scroll_capture_cancel"); // also hides the window
  }
}

function reset() {
  mode.value = "select";
  rect.value = null;
  dragging = false;
  frames.value = 0;
  heightPx.value = 0;
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await listen("selector-reset", reset);
  await listen<{ frames: number; height: number }>("scroll-progress", (e) => {
    frames.value = e.payload.frames;
    heightPx.value = e.payload.height;
  });
  await listen<string>("scroll-capture-error", () => {
    invoke("scroll_capture_cancel");
  });
});

onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<style scoped>
.veil {
  position: fixed;
  inset: 0;
  cursor: crosshair;
  user-select: none;
}
.shade {
  position: absolute;
  background: rgba(0, 0, 0, 0.3);
}
.shade.full {
  inset: 0;
}
.marquee {
  position: absolute;
  border: 1.5px dashed #fff;
  box-sizing: border-box;
}
.size-label {
  position: absolute;
  right: 4px;
  bottom: 4px;
  font: 11px -apple-system, sans-serif;
  color: #fff;
  background: rgba(0, 0, 0, 0.6);
  padding: 2px 6px;
  border-radius: 4px;
}
.instructions {
  position: absolute;
  top: 24px;
  left: 50%;
  transform: translateX(-50%);
  font: 13px -apple-system, sans-serif;
  color: #fff;
  background: rgba(0, 0, 0, 0.6);
  padding: 6px 14px;
  border-radius: 8px;
  pointer-events: none;
}
.panel {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background: #2c2c2e;
  border-radius: 10px;
  font: 12px -apple-system, sans-serif;
  color: #fff;
}
.rec-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #ff453a;
  animation: pulse 1.2s infinite;
}
@keyframes pulse {
  50% { opacity: 0.3; }
}
.panel-text {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
}
.panel button {
  border: none;
  border-radius: 6px;
  padding: 4px 12px;
  font-size: 12px;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.16);
  color: #fff;
}
.panel .done {
  background: #30d158;
  color: #1c1c1e;
  font-weight: 600;
}
</style>
```

Also add a global style in this component (unscoped block) so the page is transparent: `html, body, #app { margin: 0; background: transparent !important; }`.

- [ ] **Step 2: Verify + commit**

```bash
npm run test && npm run build
git add src/selector/SelectorApp.vue
git commit -m "feat: selector UI — drag-select veil and scroll control panel"
```

---

### Task 7: Queue persistence (TDD)

**Files:**
- Modify: `src/overlay/queue.ts`, `src/overlay/queue.test.ts`, `src/overlay/OverlayApp.vue`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Failing tests** (append to `queue.test.ts`):

```ts
describe("queue persistence", () => {
  it("serialize round-trips through restore", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    q.touch("/b.png");
    const data = q.serialize();
    const q2 = createQueue(5);
    q2.restore(data);
    expect(q2.items).toEqual(q.items.map((i) => ({ ...i })));
  });

  it("restore drops entries beyond max and tolerates garbage", () => {
    const q = createQueue(2);
    q.restore([
      { path: "/a.png", version: 1 },
      { path: "/b.png", version: 0 },
      { path: "/c.png", version: 0 },
    ]);
    expect(q.items.length).toBe(2);
    const q2 = createQueue(5);
    q2.restore("nonsense" as unknown as []);
    expect(q2.items.length).toBe(0);
  });
});
```

- [ ] **Step 2: Implement in `queue.ts`** (add to the returned object; keep existing API):

```ts
  function serialize(): CaptureItem[] {
    return items.map((i) => ({ ...i }));
  }

  function restore(data: unknown) {
    items.length = 0;
    if (!Array.isArray(data)) return;
    for (const entry of data.slice(0, max)) {
      if (entry && typeof entry.path === "string" && typeof entry.version === "number") {
        items.push({ path: entry.path, version: entry.version });
      }
    }
  }

  return { items, add, dismiss, touch, serialize, restore };
```

Run `npm run test` — all green.

- [ ] **Step 3: `filter_existing` command in `lib.rs`** (+ register):

```rust
#[tauri::command]
fn filter_existing(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}
```

- [ ] **Step 4: Wire persistence in `OverlayApp.vue`** — inside `onMounted`, BEFORE registering listeners:

```ts
const STORAGE_KEY = "digitshot.queue.v1";

try {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw) {
    const stored: { path: string; version: number }[] = JSON.parse(raw);
    const alive = await invoke<string[]>("filter_existing", {
      paths: stored.map((s) => s.path),
    });
    queue.restore(stored.filter((s) => alive.includes(s.path)));
    syncWindow();
  }
} catch (e) {
  console.warn("queue restore failed", e);
}
```

And persist on every change (after the existing `watch`):

```ts
watch(
  () => queue.items.map((i) => i.path + ":" + i.version).join("|"),
  () => localStorage.setItem(STORAGE_KEY, JSON.stringify(queue.serialize())),
);
```

- [ ] **Step 5: Verify + commit**

```bash
npm run test && npm run build && cd src-tauri && cargo check
git add src/overlay/queue.ts src/overlay/queue.test.ts src/overlay/OverlayApp.vue src-tauri/src/lib.rs
git commit -m "feat: thumbnail queue persists across restarts via localStorage + filter_existing"
```

---

### Task 8: Editor tall-image fit + README + smoke prep

**Files:**
- Modify: `src/editor/EditorApp.vue`, `README.md`

- [ ] **Step 1: Tall-fit rule.** In `EditorApp.vue` replace `fitToViewport` and add a viewport class:

```ts
const tallFit = ref(false);

function fitToViewport() {
  const snap = state.snapshot;
  if (!snap || !viewport.value) return;
  const maxW = viewport.value.clientWidth - 32;
  const maxH = viewport.value.clientHeight - 32;
  // Stitched scrolling captures are extremely tall: fitting both axes would
  // render them as a sliver. Fit width only and scroll vertically instead.
  tallFit.value = snap.baseHeight / snap.baseWidth >= 2;
  scale.value = tallFit.value
    ? Math.min(1, maxW / snap.baseWidth)
    : fitScale(snap.baseWidth, snap.baseHeight, maxW, maxH);
}
```

Template: `<div ref="viewport" class="viewport" :class="{ tall: tallFit }">`. Style: `.viewport.tall { place-items: start center; }` (centered overflow clips the top of tall content; start-aligned keeps row 0 reachable; the viewport already has `overflow: auto`).

- [ ] **Step 2: README.** Document scrolling capture: hotkeys (`Cmd+Shift+6` release / `Cmd+Shift+0` dev), tray item, the manual-scroll flow (drag region → scroll content → Done), the one-time Screen Recording permission (+ relaunch after granting, monthly macOS re-nag, dev-build re-grant after rebuilds and the stable-signing workaround), queue persistence behavior, and the tall-image editor fit.

- [ ] **Step 3: Full verification + commit**

```bash
npm run test && npm run build && cd src-tauri && cargo test && cargo check
git add src/editor/EditorApp.vue README.md
git commit -m "feat: width-fit for tall stitched images; phase 2 README"
```

Manual smoke (orchestrator + Patrick, after all tasks): trigger scrolling capture via tray → permission prompt path → re-trigger → drag region over a long page → scroll → Done → tall PNG on clipboard + thumbnail + editor width-fit; Esc/Cancel paths leave no session running; restart app → queue restored without dead entries.
