# DigitShot

Open-source CleanShot-style screenshot tool for macOS, built with Tauri 2 (Rust) + Vue 3.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Features

- **Instant area/window capture** via native `screencapture -i` — the system crosshair and window-picker UI, no Screen Recording permission required. Press Space during selection to switch to window-pick mode; Esc cancels.
- **Auto-copy + save** — captured PNG is copied to the clipboard and saved to `~/Pictures/DigitShot/<yyyy-MM-dd HH.mm.ss>.png`.
- **Floating thumbnail queue** — pinned to the bottom-left corner, visible on all Spaces and over fullscreen apps, never steals keyboard focus. Hover a card to reveal **Edit / Copy / Finder / Dismiss** actions. Queue persists across restarts; entries whose files have been deleted are silently dropped on launch.
- **Annotation editor** — rectangle tool (5 colors: red/blue/green/yellow/black; 3 stroke widths: 2/4/6 px), pixelate blur, crop, and resize with optional aspect lock. Undo/redo. Copy, Save, and Save As all export at full native (Retina) resolution.
- **Scrolling capture** — drag a region over scrollable content, click Start Capture, scroll at a normal pace, click Done. The stitcher assembles a full-resolution tall PNG and delivers it into the same pipeline (clipboard, thumbnail queue, editor). A status ring overlaid on the region turns red when the stitcher loses track (scroll back up until it turns green), and green when it is locked.
- **Menubar-only** — no Dock icon; lives entirely in the system tray.

---

## Install

### Download (macOS, unsigned)

Download the `.dmg` from [GitHub Releases](https://github.com/digitxbv/digitshot/releases) and drag **DigitShot.app** to `/Applications`.

> **Gatekeeper warning:** builds are not notarized. macOS will block the first launch with "Apple cannot check it for malicious software."
>
> Workaround — either:
> - Right-click the app in Finder → **Open** → **Open** in the dialog, or
> - Run once in Terminal: `xattr -d com.apple.quarantine /Applications/DigitShot.app`

### Build from source

**Prerequisites**

- Rust stable (`rustup` — [rustup.rs](https://rustup.rs))
- Node 18+
- Xcode Command Line Tools (`xcode-select --install`)

```bash
git clone https://github.com/digitxbv/digitshot.git
cd digitshot
npm install
npm run tauri build
```

The app bundle lands at `src-tauri/target/release/bundle/macos/DigitShot.app`. The `.dmg` is in the same directory.

---

## Usage

### Hotkeys (release build)

| Hotkey | Action |
|---|---|
| `Cmd+Shift+2` | Capture area/window — drag to select, Space to pick a window, Esc to cancel |
| `Cmd+Shift+6` | Start scrolling capture |

### Tray menu

| Item | Action |
|---|---|
| Capture Area | Same as `Cmd+Shift+2` |
| Scrolling Capture | Same as `Cmd+Shift+6` |
| Open Captures Folder | Reveals `~/Pictures/DigitShot/` in Finder |
| Quit DigitShot | Exits the app |

### Permissions

- **Regular capture** — no permissions required; `screencapture -i` handles it natively.
- **Scrolling capture** — requires Screen Recording (System Settings → Privacy & Security → Screen Recording). Grant it for DigitShot and **relaunch the app** — macOS does not apply the grant to a running process.

### Scrolling capture how-to

1. Trigger via `Cmd+Shift+6` or the tray menu.
2. Drag a selection over **only the scrolling content area** — exclude static sidebars and navigation chrome; the stitcher auto-detects sticky headers/footers but static side columns degrade match quality if included.
3. Click **Start Capture** in the control panel that appears.
4. Scroll at a steady, moderate pace. The ring overlaid on your region is:
   - **Green** — stitcher is locked on the content.
   - **Red** — stitcher lost track; scroll back up slightly until it turns green, then continue.
5. Click **Done** when you reach the bottom. The stitched PNG is saved, copied to the clipboard, and added to the thumbnail queue like any regular capture.
6. Press **Esc** or click **Cancel** at any point to discard the session.

---

## Development

```bash
npm install
npm run tauri dev
```

Dev builds use `Cmd+Shift+1` (capture) and `Cmd+Shift+0` (scrolling capture), display a **DEV** label in the menubar, and show orange accents in the editor toolbar and overlay cards — so a dev instance can run alongside an installed release build without hotkey conflicts.

**Tests**

```bash
# Frontend (Vitest)
npm run test

# Rust
cd src-tauri && cargo test
```

**Screen Recording caveat for dev builds:** unsigned binaries lose the Screen Recording TCC grant after each rebuild. Re-grant in System Settings and relaunch, or code-sign the dev binary with a stable local identity.

---

## How it works

Regular capture delegates entirely to `/usr/sbin/screencapture -i`, which provides the native crosshair and window-pick UI without requiring Screen Recording permission. The floating thumbnail overlay and the scrolling-capture selector are non-activating `NSPanel` windows (via [tauri-nspanel](https://github.com/ahkohd/tauri-nspanel)) configured with `CanJoinAllSpaces` and `FullScreenAuxiliary` collection behaviors — they float over fullscreen apps on every Space without ever stealing key focus. The annotation editor renders the screenshot as a [Konva.js](https://konvajs.org) stage background with annotation shapes on top; Copy/Save/Save As flatten the stage to a canvas at 1:1 native resolution before encoding to PNG.

Scrolling capture records the selected region continuously (paced by the screen-grab call itself, typically 10–20 fps) via `xcap::Monitor::capture_region`. Each frame is fed to the stitcher (`src-tauri/src/stitch.rs`), a pure Rust engine that matches consecutive frames using edge-domain normalized cross-correlation (NCC) with a coarse-to-fine search: frames wider than ~480 px are first matched on a downscaled copy, then refined at full resolution in a narrow row window — keeping Retina-resolution stitching fast. A sticky header/footer detector strips UI chrome that appears in every frame so it appears only once in the output. Static sidebar columns are detected from the first scrolled pair and excluded from the match region. The finished canvas is saved as a single tall PNG.

Full design documentation: [`docs/superpowers/specs/`](docs/superpowers/specs/).

---

## Known limitations

- Scrolling capture operates on the primary monitor only.
- Blur regions are fixed once drawn — select and delete the region, then redraw to adjust.
- Hotkeys are not configurable (hardcoded in `src-tauri/src/lib.rs`).
- Builds are unsigned and not notarized; see Gatekeeper workaround above.

---

## License

MIT — Copyright (c) 2026 [DigitX B.V.](https://digitx.nl) — see [LICENSE](LICENSE).
