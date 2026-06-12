# DigitShot — Phase 2 Design: Scrolling Capture + Queue Persistence

Date: 2026-06-12
Status: Approved by Patrick

## Goal

1. **Scrolling capture** (manual-scroll v1): capture a user-selected region while the
   user scrolls the content; stitch frames into one tall PNG that enters the normal
   pipeline (clipboard, thumbnail queue, editor).
2. **Queue persistence**: the thumbnail queue survives app restarts.
3. **Editor tall-image handling**: stitched output is usable in the editor.

## Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Frame capture backend | `xcap` crate `Monitor::capture_region` at ~3fps | `screencapture -R` has TCC attribution traps + wallpaper-only silent failures; app needs the Screen Recording grant either way (research 2026-06-12) |
| Scroll mode | User scrolls manually; v1 has no synthetic scroll events | Reliability; no Accessibility permission |
| Permission preflight | `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` via direct FFI | Two C calls; no plugin dependency |
| Hotkeys | Scrolling capture: release `Cmd+Shift+6`, dev `Cmd+Shift+0`; also a tray item | 3/4/5 reserved by macOS; matches existing release/dev split |
| Selector/panel UI | ONE extra window (`selector`) that morphs: fullscreen veil for drag-select → small floating control panel during capture | Single window lifecycle; panel positioned outside the region so it is never captured |
| Stitching | Pure Rust (`image` + `imageproc`): Sobel + NCC template match, inertia window, sticky-edge trim | Modeled on long-shot-rs + ShareX; fully unit-testable |
| Queue persistence | Overlay `localStorage` + `filter_existing` Rust command on restore | Dev/release have different webview origins → separate queues for free |
| Monitor support | Primary monitor only (v1) | YAGNI |

## Architecture

### Rust

- `permissions.rs` — `has_screen_recording()`, `request_screen_recording()` (FFI to
  CoreGraphics). No TCC prompt is triggered by preflight; request triggers the system
  dialog once.
- `stitch.rs` — pure stitching engine, no I/O, no Tauri types:
  `Stitcher::new(config) → push_frame(&RgbaImage) → PushResult {Appended(rows) | SkippedDuplicate | SkippedLowConfidence}`,
  `finish() → RgbaImage`. Algorithm details live in the implementation plan.
- `scrolling.rs` — session state machine guarded by a mutex (one session at a time):
  `start(region)` spawns the ~350ms capture loop thread feeding the Stitcher and
  emitting `scroll-frame-count {frames, height}` events; `stop()` finishes the stitch,
  writes the PNG via the existing filename/captures-dir helpers, copies to clipboard,
  emits `capture-taken`; `cancel()` discards.
- Commands: `scroll_capture_begin` (permission check → show selector window),
  `scroll_capture_start(region)`, `scroll_capture_stop`, `scroll_capture_cancel`,
  `position_scroll_panel(region)` (shrink/move selector window outside the region),
  `filter_existing(paths) → Vec<String>`.
- Hotkey + tray item trigger the same `scroll_capture_begin` path as the command.

### Frontend

- New entry `selector.html` → `src/selector/SelectorApp.vue`, two modes:
  - **select**: fixed full-screen veil (`rgba(0,0,0,0.3)`), drag rectangle (clear
    cut-out + white dashed border, size label), Esc cancels (closes/hides window),
    mouse-up ≥ 24×24 px → invoke `scroll_capture_start` + `position_scroll_panel`,
    switch to panel mode.
  - **panel**: compact dark pill: "Scroll the content" · live frame/height counter
    (from `scroll-frame-count`) · **Done** · **Cancel**.
- Overlay: queue serialized to `localStorage` on every mutation; on mount, restore
  through `filter_existing`, then `syncWindow()`.
- Editor: `fitScale` rule — if `imgH / imgW ≥ 2`, fit width only (vertical scroll in
  the existing `overflow: auto` viewport); else fit both axes as today.

### Selector window properties

Borderless, transparent, always-on-top NSPanel (same treatment as overlay: non-
activating, all-spaces) sized to the primary monitor work area in select mode.
Hidden (not destroyed) between sessions.

## Permission flow

`scroll_capture_begin`: preflight → granted: show selector. Not granted: call
request (system dialog appears once per identity), emit `scroll-permission-needed`;
frontend overlay shows a dialog directing to System Settings → Screen Recording and
noting the relaunch requirement. Never start a session without the grant (xcap would
silently produce wallpaper-only frames).

## Error handling

- Second trigger while a session runs → ignored (mutex held).
- Capture loop error (monitor gone, xcap failure) → cancel session, emit
  `scroll-capture-error {message}`, panel shows it briefly then closes.
- Done with only one frame / nothing stitched → still produces that single frame
  as the PNG (never an error, never a corrupt image).
- Stop writes file before clipboard; clipboard failure is non-fatal (as phase 1).

## Testing

- `stitch.rs`: TDD against synthetic frames (generated gradients/noise with known
  scroll offsets): exact-offset append, duplicate skip, low-confidence skip,
  sticky-header trim, single-frame finish, inertia-window rejection of a repeated
  pattern outside the window.
- `filter_existing`: unit test with tempdir files.
- Queue persistence: unit tests for serialize/restore logic (storage mocked at the
  narrow load/save boundary only).
- Capture loop, permission UX, selector UI: manual smoke.

## Out of scope (later)

Auto-scroll, multi-monitor selection, horizontal stitching, live stitched preview,
configurable hotkeys/fps.

## Known risks

- xcap uses the deprecated CG capture path → macOS 15.1+ may show periodic "bypass
  screen recording" style nags; migration path is the `scap` (ScreenCaptureKit) crate.
- Unsigned dev rebuilds reset the Screen Recording grant (TCC keys on code signature).
  README documents signing the dev binary with a stable identity as the workaround.
- `xcap::Monitor::capture_region` coordinate space (logical vs physical) must be
  verified empirically on Retina in the first implementation task.
