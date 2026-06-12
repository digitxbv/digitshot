# DigitShot

Personal CleanShot X-style screenshot tool for macOS. Menubar-only app (no Dock icon) built with Tauri 2 + Vue 3 + Konva.js.

## Phase 1 features

**Capture**

- Global hotkey `Cmd+Shift+2` triggers macOS native interactive capture: drag to select a region, Space to pick a window, Esc to cancel.
- On success: PNG is saved to `~/Pictures/DigitShot/<yyyy-MM-dd HH.mm.ss>.png` and auto-copied to the clipboard.

**Thumbnail queue**

- Floating overlay pinned to the bottom-left of the primary screen, visible on all Spaces and over fullscreen apps, never steals key focus.
- Newest capture appears at the bottom; max 5 thumbnails at once.
- Hover a card to reveal action buttons: **Edit · Copy · Finder · ✕ (Dismiss)**. Clicking a card opens the editor.
- Thumbnails survive app restarts; any entry whose file has been deleted is silently dropped on launch.

**Tray menu**

- **Capture Area** — same as the hotkey.
- **Scrolling Capture** — starts a scrolling capture session (see below).
- **Open Captures Folder** — reveals `~/Pictures/DigitShot/` in Finder.
- **Quit DigitShot**

**Editor**

Opens per capture. Tools:

| Tool | Notes |
|---|---|
| Select | Move and resize committed shapes via drag handles. |
| Rectangle | Draw a stroke rectangle; pick color (red/blue/green/yellow/black) and stroke width (2/4/6 px). |
| Blur | Draw a pixelated blur region over the image. Fixed once committed — delete and redraw to adjust. |
| Crop | Drag to define crop region, then confirm or cancel via the bar that appears. |

Resize dialog (`Resize…` button): set output dimensions in pixels, with optional aspect-lock. Capped at 4× the current size.

Images taller than a 2:1 aspect ratio (typical of stitched scrolling captures) are fit to the viewport width and scroll vertically, keeping the full image reachable.

Keyboard shortcuts in the editor:

| Shortcut | Action |
|---|---|
| `Cmd+Z` | Undo |
| `Shift+Cmd+Z` | Redo |
| `Delete` / `Backspace` | Remove selected shape |
| `Esc` | Cancel crop/draft → deselect → close window |

Actions: **Copy** (flattened PNG to clipboard), **Save** (overwrite original), **Save As…** — all export at full native resolution.

## Phase 2 features

**Scrolling capture**

Trigger via the tray item **Scrolling Capture**, or the global hotkey:

- Release build: `Cmd+Shift+6`
- Dev build: `Cmd+Shift+0` (3, 4, and 5 are reserved by macOS)

**Flow:**

1. Drag a region over the scrollable content you want to capture.
2. A control panel appears — scroll the content yourself at a steady pace.
3. Click **Done** when you reach the bottom.
4. The stitched tall PNG is automatically saved to `~/Pictures/DigitShot/`, copied to the clipboard, and added to the thumbnail queue — exactly like a regular capture.
5. Press **Esc** or click **Cancel** at any point to discard the session.

**Permissions:**

Scrolling capture requires the Screen Recording TCC permission (System Settings → Privacy & Security → Screen Recording). Grant it for DigitShot and relaunch the app — macOS does not apply the grant to a running process. macOS may periodically re-prompt to confirm the permission. Dev builds lose the Screen Recording grant after each rebuild unless the dev binary is code-signed with a stable identity.

## Architecture

Capture is delegated entirely to macOS via `screencapture -i`, which provides the native crosshair and window-pick UI without requiring a Screen Recording TCC permission. The floating thumbnail overlay is a non-activating `NSPanel` (via `tauri-nspanel`) configured with `CanJoinAllSpaces` and `FullScreenAuxiliary` collection behaviors, so it floats over fullscreen apps on every Space without ever taking key focus. The editor renders the screenshot as a Konva.js stage background with annotation shapes on top; Copy/Save/Save As flatten the Konva stage to a canvas at 1:1 (native) resolution before encoding to PNG. Crop and Resize both flatten-and-replace the base image, which bakes all current annotations into the new base (undo still restores the pre-operation state).

Reference: [`docs/superpowers/specs/`](docs/superpowers/specs/) for the phase 1 design doc and [`docs/superpowers/plans/`](docs/superpowers/plans/) for implementation plans.

## Known limitations

- Blur regions are fixed once drawn; to adjust, select and delete the region, then redraw.
- Only one editor window at a time (a second capture reuses the existing window).
- Hotkeys are not configurable; they are hardcoded in `src-tauri/src/lib.rs`.
- Crop and Resize bake current annotations into the image; undo still restores the pre-operation state.

## Development

```bash
npm install
npm run tauri dev
```

Dev builds (`npm run tauri dev`) use `Cmd+Shift+1`, show a **DEV** menubar label next to the tray icon, and display orange accents in the editor toolbar and overlay cards — so a dev instance can run alongside the installed release app (which uses `Cmd+Shift+2`) without conflict.

**Tests**

```bash
# Frontend (Vitest)
npm run test

# Rust
cd src-tauri && cargo test
```

**Production build**

```bash
npm run tauri build
```

## License

MIT — see [LICENSE](LICENSE).
