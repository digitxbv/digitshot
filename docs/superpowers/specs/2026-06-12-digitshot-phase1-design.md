# DigitShot — Phase 1 Design

Date: 2026-06-12
Status: Approved by Patrick

## Goal

A personal CleanShot X-style screenshot tool for macOS: hotkey-triggered region/window
capture, CleanShot-style floating thumbnail queue in the lower-left corner, and a quick
annotation editor (rectangle, blur, crop, resize).

## Decisions made

| Decision | Choice | Rationale |
|---|---|---|
| Stack | Tauri v2 (Rust + web UI) | User preference; canvas-based editor is fastest in web tech |
| Frontend | Vue 3 + Vite + TypeScript | Matches user's ecosystem |
| Canvas library | Konva.js (vue-konva) | Mature, good for shape/transform/crop editors |
| Capture mechanism | Spawn `/usr/sbin/screencapture -i <path>` | Native crosshair + window-pick UX for free; no Screen Recording permission |
| Hotkey | `Cmd+Shift+2` (hardcoded, single constant) | No conflict with system `Cmd+Shift+4` |
| Post-capture | Auto-copy to clipboard + thumbnail + save to `~/Pictures/DigitShot/` | User choice |
| Overlay window | `tauri-nspanel` non-activating panel | Float over fullscreen apps, all Spaces, no focus steal |
| App style | Menubar-only (tray icon, ActivationPolicy::Accessory) | CleanShot-like; no Dock icon |

## Architecture

Three UI surfaces, one Rust core.

### Rust core (src-tauri)
- Registers global shortcut `Cmd+Shift+2` via `tauri-plugin-global-shortcut`.
- `capture_interactive()` command: ensures `~/Pictures/DigitShot/` exists, spawns
  `screencapture -i <dir>/<yyyy-MM-dd HH.mm.ss>.png`, waits for exit.
  - File exists after exit → copy image to clipboard (arboard), emit `capture-taken { path }`.
  - No file (user pressed Esc) → silent no-op.
- Tray: **Capture area** (same command), **Open captures folder**, **Quit**.
- Window management commands: show/position overlay panel, open editor window for a path.

### Thumbnail overlay (panel window)
- Frameless, transparent, non-activating NSPanel pinned to lower-left of the main screen,
  visible on all Spaces and over fullscreen apps.
- Vertical stack of recent captures (newest at bottom, CleanShot-style), max 5 visible.
- Hover actions per thumbnail: **Edit · Copy · Show in Finder · Dismiss**. Click = Edit.
- Thumbnails persist until dismissed. Panel hides itself when the queue is empty.
- Click-through is NOT needed; panel is interactive but never takes key focus.

### Editor (normal window)
- Opens per capture (`?path=<file>`), Konva.js stage with the screenshot as background.
- Tools: select/move · rectangle (stroke color, width) · blur (pixelated region) ·
  crop · resize (output px dimensions, aspect-locked toggle) · undo/redo.
- Actions: **Copy** (flattened PNG to clipboard), **Save** (overwrite original),
  **Save As**. Save re-emits `capture-updated { path }` so the thumbnail refreshes.
- Blur is implemented as pixelation of the underlying image region (privacy-safe at
  export: baked into the flattened bitmap).

## Data flow

```
Cmd+Shift+2 ─→ Rust: screencapture -i ─→ PNG in ~/Pictures/DigitShot/
                       │ (file exists)
                       ├─→ clipboard (arboard)
                       └─→ emit capture-taken ─→ overlay prepends thumbnail
                                                     │ click/Edit
                                                     └─→ editor window (?path=…)
                                                           └─ Save → capture-updated → thumbnail refresh
```

## Error handling
- Esc during capture: no file → no event, no error.
- Hotkey registration fails (rare): tray notification; tray "Capture area" still works.
- Clipboard copy fails: log, continue — thumbnail and file are unaffected.
- Editor load fails (file deleted): show error state in editor, dismiss thumbnail.

## Testing
- Vitest: editor pure logic — crop rect math, resize dimension math, pixelation block
  mapping, undo/redo stack core behavior.
- Rust `#[cfg(test)]`: capture result handling (file-exists vs Esc path), filename
  generation.
- Manual smoke: hotkey, overlay behavior over fullscreen apps, clipboard paste.
- Mocks only at external boundaries (no real `screencapture` spawn in tests).

## Out of scope (later phases)
Pin-to-screen, scrolling capture, video/GIF recording, cloud upload, settings UI,
configurable hotkey, text/arrow/ellipse/highlight tools, multi-display overlay placement.

## Known risks
- `tauri-nspanel` is a community plugin (actively maintained as of 2026-06). Fallback:
  regular always-on-top window (loses over-fullscreen + no-focus-steal behavior).
- Subprocess TCC attribution on macOS 15+: `screencapture -i` from a dev (unsigned)
  build must be validated first thing during implementation (it is exempt from Screen
  Recording TCC, but verify on this machine).
