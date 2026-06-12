# CLAUDE.md

DigitShot — CleanShot-style macOS screenshot tool. Tauri 2 (Rust) + Vue 3 + TS + Konva.

## Commands

```bash
npm run tauri dev      # run the app (NEVER start a second one — vite port 1420 is strictPort)
npm run test           # frontend tests (Vitest)
cd src-tauri && cargo test   # Rust tests (stitcher + capture)
npm run tauri build    # release bundle → src-tauri/target/release/bundle/
```

## Architecture

Four webview windows (multi-page Vite: `index.html`, `editor.html`, `selector.html`, `frame.html`):
- **overlay** (`src/overlay/`) — thumbnail queue, non-activating NSPanel, bottom-left
- **editor** (`src/editor/`) — Konva annotation editor, normal window, created on demand
- **selector** (`src/selector/`) — scrolling-capture drag-select veil → control panel (one window, three modes)
- **frame** (`src/frame/`) — click-through ring around the scroll region during capture

Rust (`src-tauri/src/`): `lib.rs` (commands, tray, hotkeys, panels), `capture.rs` (screencapture spawn), `clipboard.rs` (arboard), `stitch.rs` (pure stitching engine, fully unit-tested), `scrolling.rs` (capture session thread), `permissions.rs` (Screen Recording TCC preflight).

## Critical gotchas (each one cost real debugging time)

- **Canvas taint (WKWebView)**: the editor MUST load bitmaps via the `read_image` command → blob URL. NEVER `convertFileSrc` for anything drawn to a canvas — WebKit taints it regardless of CORS headers (WebKit bug 201180) and `toDataURL`/filters throw "The operation is insecure". `convertFileSrc` is fine for plain `<img>` (overlay thumbnails).
- **Dev opt-levels are load-bearing** (`src-tauri/Cargo.toml`): `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 2`. At opt-0 the stitcher's pixel loops are ~50× slower and scrolling capture appears broken. If image work seems mysteriously slow, check opt-levels before touching algorithms.
- **App lifecycle**: the `RunEvent::ExitRequested` handler in `lib.rs` calls `prevent_exit()` when `code.is_none()`. Removing it makes the app quit when the last window hides (e.g. dismissing the final thumbnail). Only tray Quit (`app.exit(0)`) may terminate.
- **tauri-nspanel**: pinned to git branch `v2.1`. Builder enums use `.value()`, not `.into()`. Call `to_panel()` at most once per window. Requires `macOSPrivateApi: true` + the `macos-private-api` Cargo feature.
- **Editor export guard**: any transient overlay drawn on the Konva stage (selection outlines, crop dim rects, …) must be hidden behind the `exporting` ref in `EditorApp.vue`, or it bakes into saved/copied PNGs.
- **Transformer timing**: watchers that attach the Konva Transformer to freshly created nodes need `{ flush: "post" }`.
- **Stitcher tests are pixel-exact by design** (`stitch.rs`): synthetic frames are literal crops of one source, so `assert_eq!(out, expected)` catches off-by-one delta math. Never weaken these assertions to make a change pass.
- **Dev vs release**: hotkeys are `Cmd+Shift+1`/`Cmd+Shift+0` in dev, `Cmd+Shift+2`/`Cmd+Shift+6` in release (`cfg!(debug_assertions)`); dev shows a DEV menubar label + orange accents so both can run simultaneously.
- **Screen Recording TCC**: every dev rebuild changes the unsigned binary's signature and silently resets the grant — scrolling capture then returns wallpaper-only frames with NO error. Re-grant + relaunch, or sign dev builds with a stable identity.

## Event contract (Rust → webviews)

`capture-taken {path}`, `capture-updated {path}`, `scroll-region {x,y,width,height}`, `scroll-progress {frames,height}`, `scroll-status {state: "locked"|"lost"}`, `scroll-capture-error <string>`, `selector-reset`, `editor-load {path}`.

## Conventions

- Pure logic (stitching, geometry, history, queue) lives in plain modules with unit tests; UI components stay thin. Mock only the Tauri IPC boundary.
- Coordinates: selector/frame work in logical px relative to the primary monitor; captured frames and the stitcher are physical px (`xcap` takes logical in, returns physical out).
- Design docs and implementation plans live in `docs/superpowers/`.
