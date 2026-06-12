# DigitShot Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** macOS menubar screenshot tool: `Cmd+Shift+2` → native interactive capture → auto-copy + floating thumbnail queue (lower-left, CleanShot-style) → canvas annotation editor (rect, blur, crop, resize).

**Architecture:** Tauri v2 menubar-only app. Rust spawns `/usr/sbin/screencapture -i` for capture, copies PNG to clipboard via arboard, emits events. Overlay window is converted to a non-activating NSPanel (tauri-nspanel) pinned bottom-left. Editor is a normal window with a Konva.js stage; crop/resize use a flatten-and-replace pattern with a snapshot history stack.

**Tech Stack:** Tauri 2.x (Rust), Vue 3 + TypeScript + Vite (multi-page: `index.html` overlay, `editor.html` editor), Konva 10 + vue-konva 3.4, arboard 3.6 (`image-data` feature), `image` 0.25, chrono, tauri-nspanel (git, branch `v2.1`), plugins: global-shortcut, opener, dialog. Tests: Vitest (TS pure logic), `cargo test` (Rust pure logic).

**Spec:** `docs/superpowers/specs/2026-06-12-digitshot-phase1-design.md`

**Conventions for all tasks:**
- Working dir is repo root `/Users/patrickgerrits/Development/DigitShot` unless stated.
- Commit after every task with the message given. End every commit message with:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` (blank line before it).
- Mock nothing except the Tauri IPC boundary; pure-logic modules must be testable with no mocks.
- `npm run tauri dev` is the manual smoke loop; `cargo test` runs inside `src-tauri/`.

---

### Task 1: Scaffold Tauri 2 + Vue 3 + TS project

**Files:**
- Create: entire app via scaffolder, then modify `vite.config.ts`, `tauri.conf.json`, `src-tauri/capabilities/default.json`, `package.json`, `editor.html`, `vitest` setup
- Delete: template demo content (`src/components/`, greet code)

- [ ] **Step 1: Scaffold into the existing repo**

The repo root already contains `.git/`, `.claude/`, `docs/`. Scaffold into a temp dir and move contents in:

```bash
cd /Users/patrickgerrits/Development/DigitShot
npm create tauri-app@latest digitshot-tmp -- --template vue-ts --yes
rsync -a --exclude .git digitshot-tmp/ ./
rm -rf digitshot-tmp
npm install
```

If `--yes`/`--template` flags fail, fall back to interactive prompts: name `digitshot`, TypeScript, npm, Vue, TypeScript.

- [ ] **Step 2: Install frontend deps**

```bash
npm install konva vue-konva @tauri-apps/plugin-dialog @tauri-apps/plugin-opener
npm install -D vitest
```

- [ ] **Step 3: Multi-page Vite + Vitest config**

Replace `vite.config.ts` (keep any Tauri-specific server settings the template generated — merge, don't drop `clearScreen`/`server` blocks):

```ts
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        editor: resolve(__dirname, "editor.html"),
      },
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
}));
```

Add to `package.json` scripts: `"test": "vitest run"`.

- [ ] **Step 4: Create `editor.html` next to `index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>DigitShot Editor</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/editor/main.ts"></script>
  </body>
</html>
```

Create placeholder `src/editor/main.ts`:

```ts
import { createApp } from "vue";
import VueKonva from "vue-konva";
import EditorApp from "./EditorApp.vue";

createApp(EditorApp).use(VueKonva).mount("#app");
```

And placeholder `src/editor/EditorApp.vue`:

```vue
<template>
  <div>editor placeholder</div>
</template>
```

- [ ] **Step 5: Configure `src-tauri/tauri.conf.json`**

Set these fields (keep generated `build` section as-is):

```json
{
  "productName": "DigitShot",
  "identifier": "nl.digitx.digitshot",
  "app": {
    "macOSPrivateApi": true,
    "windows": [
      {
        "label": "overlay",
        "url": "index.html",
        "title": "DigitShot",
        "visible": false,
        "transparent": true,
        "decorations": false,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "shadow": false,
        "acceptFirstMouse": true,
        "width": 260,
        "height": 180
      }
    ],
    "security": {
      "csp": null,
      "assetProtocol": {
        "enable": true,
        "scope": ["$HOME/Pictures/DigitShot/**"]
      }
    }
  }
}
```

- [ ] **Step 6: Capabilities**

Replace `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "DigitShot windows",
  "windows": ["overlay", "editor"],
  "permissions": [
    "core:default",
    "core:window:allow-close",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-focus"
  ]
}
```

(Permissions for plugins not yet installed would fail `cargo check` — `dialog:default`, `opener:default`, and `global-shortcut:allow-register` are added in Task 3 together with their plugins.)

- [ ] **Step 7: Strip template demo**

In `src/App.vue`, replace contents with a minimal placeholder (overlay UI comes in Task 5):

```vue
<template>
  <div></div>
</template>
```

Delete `src/components/Greet.vue` (or whatever demo component exists) and the `greet` command from `src-tauri/src/lib.rs` (leave a bare builder that compiles).

- [ ] **Step 8: Verify build + tests run**

```bash
npm run test            # expected: "no test files found" exit 0 (or passWithNoTests; add --passWithNoTests to the script if vitest errors)
cd src-tauri && cargo check && cd ..
```

Expected: `cargo check` finishes without errors.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: scaffold Tauri 2 + Vue 3 + TS app with overlay window and editor entry"
```

---

### Task 2: Rust capture module (pure logic, TDD)

**Files:**
- Create: `src-tauri/src/capture.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod capture;`), `src-tauri/Cargo.toml`

- [ ] **Step 1: Add dependencies to `src-tauri/Cargo.toml`**

```toml
[dependencies]
# ...existing tauri deps stay...
chrono = "0.4"
arboard = { version = "3", features = ["image-data"] }
image = "0.25"
base64 = "0.22"
urlencoding = "2"
```

Also ensure the tauri dep has the needed features:

```toml
tauri = { version = "2", features = ["macos-private-api", "tray-icon"] }
```

- [ ] **Step 2: Write failing tests in `src-tauri/src/capture.rs`**

```rust
use std::path::{Path, PathBuf};

pub fn captures_dir(home: &Path) -> PathBuf {
    home.join("Pictures").join("DigitShot")
}

pub fn capture_filename(now: &chrono::DateTime<chrono::Local>) -> String {
    format!("DigitShot {}.png", now.format("%Y-%m-%d at %H.%M.%S"))
}

/// Interprets the result of a `screencapture -i` run.
/// Esc during capture exits 0 but writes no file -> None.
pub fn capture_result(exit_success: bool, file_exists: bool, path: PathBuf) -> Option<PathBuf> {
    if exit_success && file_exists {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn filename_is_sortable_and_finder_safe() {
        let t = chrono::Local.with_ymd_and_hms(2026, 6, 12, 9, 41, 7).unwrap();
        assert_eq!(capture_filename(&t), "DigitShot 2026-06-12 at 09.41.07.png");
    }

    #[test]
    fn captures_dir_is_under_pictures() {
        assert_eq!(
            captures_dir(Path::new("/Users/x")),
            PathBuf::from("/Users/x/Pictures/DigitShot")
        );
    }

    #[test]
    fn esc_during_capture_yields_none() {
        // screencapture exits 0 on Esc but writes nothing
        assert_eq!(capture_result(true, false, PathBuf::from("/tmp/a.png")), None);
        assert!(capture_result(true, true, PathBuf::from("/tmp/a.png")).is_some());
    }
}
```

Add `mod capture;` to `src-tauri/src/lib.rs`.

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test
```

Expected: 3 passed. (The code above is implementation + tests together — the functions are small enough that test-first happens within one file write; verify all three pass.)

- [ ] **Step 4: Add the (untested, thin) subprocess wrapper to `capture.rs`**

```rust
/// Spawns the native interactive capture UI. Blocks until the user
/// finishes (drag/window-pick) or cancels with Esc.
pub fn run_interactive_capture(dir: &Path) -> std::io::Result<Option<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(capture_filename(&chrono::Local::now()));
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .arg("-i")
        .arg(&path)
        .status()?;
    Ok(capture_result(status.success(), path.exists(), path))
}
```

- [ ] **Step 5: `cargo test && cargo check`, then commit**

```bash
git add -A
git commit -m "feat: capture module — filename generation and screencapture result handling"
```

---

### Task 3: Rust core — clipboard, commands, hotkey, tray

**Files:**
- Create: `src-tauri/src/clipboard.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`, `src-tauri/capabilities/default.json`

- [ ] **Step 1: Add plugins to `src-tauri/Cargo.toml`**

```toml
tauri-plugin-global-shortcut = "2"
tauri-plugin-opener = "2"
tauri-plugin-dialog = "2"
```

Add to the capabilities `permissions` array in `src-tauri/capabilities/default.json`:

```json
"global-shortcut:allow-register",
"dialog:default",
"opener:default"
```

- [ ] **Step 2: Create `src-tauri/src/clipboard.rs`**

```rust
use std::borrow::Cow;
use std::path::Path;

fn copy_rgba(width: u32, height: u32, bytes: Vec<u8>) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_image(arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(bytes),
    })
    .map_err(|e| e.to_string())
}

pub fn copy_png_file(path: &Path) -> Result<(), String> {
    let img = image::open(path).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    copy_rgba(w, h, img.into_raw())
}

pub fn copy_png_bytes(png: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(png).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    copy_rgba(w, h, img.into_raw())
}
```

- [ ] **Step 3: Rewrite `src-tauri/src/lib.rs`**

```rust
mod capture;
mod clipboard;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
struct CapturePayload {
    path: String,
}

fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").expect("HOME not set"))
}

/// Runs the interactive capture on a background thread; on success copies the
/// image to the clipboard (best-effort) and notifies the overlay.
fn trigger_capture(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let dir = capture::captures_dir(&home_dir());
        match capture::run_interactive_capture(&dir) {
            Ok(Some(path)) => {
                if let Err(e) = clipboard::copy_png_file(&path) {
                    eprintln!("clipboard copy failed: {e}");
                }
                let _ = app.emit(
                    "capture-taken",
                    CapturePayload { path: path.to_string_lossy().into_owned() },
                );
            }
            Ok(None) => {} // Esc — silent no-op
            Err(e) => eprintln!("capture failed: {e}"),
        }
    });
}

#[tauri::command]
fn capture_interactive(app: AppHandle) {
    trigger_capture(&app);
}

#[tauri::command]
fn copy_image(path: String) -> Result<(), String> {
    clipboard::copy_png_file(std::path::Path::new(&path))
}

#[tauri::command]
fn copy_image_data(base64_png: String) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_png)
        .map_err(|e| e.to_string())?;
    clipboard::copy_png_bytes(&bytes)
}

#[tauri::command]
fn save_png(app: AppHandle, path: String, base64_png: String) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_png)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let _ = app.emit("capture-updated", CapturePayload { path });
    Ok(())
}

#[tauri::command]
fn reveal_in_finder(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_editor(app: AppHandle, path: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("editor") {
        win.emit("editor-load", CapturePayload { path }).map_err(|e| e.to_string())?;
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
    } else {
        let url = format!("editor.html?path={}", urlencoding::encode(&path));
        tauri::WebviewWindowBuilder::new(&app, "editor", tauri::WebviewUrl::App(url.into()))
            .title("DigitShot")
            .inner_size(1160.0, 800.0)
            .build()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            capture_interactive,
            copy_image,
            copy_image_data,
            save_png,
            reveal_in_finder,
            open_editor,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Global shortcut: Cmd+Shift+2
            {
                use tauri_plugin_global_shortcut::{
                    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
                };
                let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Digit2);
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, sc, event| {
                            if sc == &shortcut && event.state() == ShortcutState::Pressed {
                                trigger_capture(app);
                            }
                        })
                        .build(),
                )?;
                if let Err(e) = app.global_shortcut().register(shortcut) {
                    eprintln!("hotkey registration failed: {e}");
                }
            }

            // Tray
            let capture_item = MenuItem::with_id(app, "capture", "Capture Area", true, None::<&str>)?;
            let folder_item =
                MenuItem::with_id(app, "folder", "Open Captures Folder", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit DigitShot", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&capture_item, &folder_item, &quit_item])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "capture" => trigger_capture(app),
                    "folder" => {
                        use tauri_plugin_opener::OpenerExt;
                        let dir = capture::captures_dir(&home_dir());
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Compile and manual smoke test**

```bash
cd src-tauri && cargo check && cd ..
npm run tauri dev
```

Manual checks (CRITICAL — validates the TCC risk from the spec):
1. Menubar icon appears; no Dock icon.
2. Tray → Capture Area → native crosshair appears, drag a region → PNG lands in `~/Pictures/DigitShot/`, Cmd+V in Notes pastes the image.
3. Press Esc during capture → no file, no error.
4. `Cmd+Shift+2` triggers the same flow. If it doesn't, note whether macOS shows an Accessibility/Input-Monitoring prompt for the terminal and report it — do not silently skip.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: capture pipeline — hotkey, tray, clipboard copy, editor/save commands"
```

---

### Task 4: NSPanel overlay window (Rust side)

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`

- [ ] **Step 1: Add dependency**

```toml
tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", branch = "v2.1" }
```

- [ ] **Step 2: Panel setup in `lib.rs`**

Add near the top of the file:

```rust
#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}
```

In `run()`, register the plugin on the builder:

```rust
.plugin(tauri_nspanel::init())
```

In `setup`, AFTER the tray block, convert the overlay window (call `to_panel` exactly once):

```rust
#[cfg(target_os = "macos")]
{
    let window = app.get_webview_window("overlay").unwrap();
    let panel = window.to_panel::<OverlayPanel>().unwrap();
    panel.set_level(PanelLevel::Status.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .into(),
    );
    panel.set_hides_on_deactivate(false);
}
```

(If `set_hides_on_deactivate` doesn't exist on the panel handle in branch v2.1, check the trait methods with `cargo doc` or the repo's `panel.rs`; the call may be named differently — the requirement is `hidesOnDeactivate = false` so the overlay survives app switches.)

- [ ] **Step 3: Overlay show/hide/resize commands**

Add commands (and register all three in `generate_handler!`):

```rust
const OVERLAY_MARGIN: i32 = 16;

/// Resize the overlay to fit its content and pin it bottom-left of the
/// primary monitor's work area. Runs window ops on the main thread.
#[tauri::command]
fn resize_overlay(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let win = app.get_webview_window("overlay").ok_or("no overlay window")?;
    let monitor = win
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("no primary monitor")?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let phys_w = (width * scale) as u32;
    let phys_h = (height * scale) as u32;
    let margin = (OVERLAY_MARGIN as f64 * scale) as i32;
    let x = work.position.x + margin;
    let y = work.position.y + work.size.height as i32 - phys_h as i32 - margin;
    win.set_size(tauri::PhysicalSize::new(phys_w, phys_h)).map_err(|e| e.to_string())?;
    win.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn show_overlay(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let panel = app.get_webview_panel("overlay").map_err(|e| format!("{e:?}"))?;
        panel.show(); // orderFrontRegardless — never steals focus
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = app.get_webview_window("overlay") { let _ = w.show(); }
    }
    Ok(())
}

#[tauri::command]
fn hide_overlay(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let panel = app.get_webview_panel("overlay").map_err(|e| format!("{e:?}"))?;
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = app.get_webview_window("overlay") { let _ = w.hide(); }
    }
    Ok(())
}
```

Note: `work_area()` returns a rect whose `position` is already in global physical coordinates on the v2 Monitor API — verify the exact field shapes against the installed tauri version (`cargo doc -p tauri --open`, `Monitor::work_area`). If `work_area()` is unavailable in the pinned version, fall back to `monitor.size()`/`monitor.position()` and use a 40px bottom margin to clear the Dock heuristically — but prefer `work_area`.

- [ ] **Step 4: Compile, run, verify**

```bash
cd src-tauri && cargo check && cd ..
```

Manual: app still launches; overlay window is invisible (it's empty and hidden — full verification happens in Task 5 when the frontend calls `show_overlay`).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: overlay NSPanel — non-activating, all-spaces, bottom-left anchoring"
```

---

### Task 5: Overlay frontend — thumbnail queue

**Files:**
- Create: `src/overlay/queue.ts`, `src/overlay/queue.test.ts`, `src/overlay/OverlayApp.vue`
- Modify: `src/App.vue`, `src/main.ts`

- [ ] **Step 1: Write failing tests `src/overlay/queue.test.ts`**

```ts
import { describe, it, expect } from "vitest";
import { createQueue } from "./queue";

describe("capture queue", () => {
  it("prepends new captures, newest first", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    expect(q.items.map((i) => i.path)).toEqual(["/b.png", "/a.png"]);
  });

  it("caps visible items, dropping the oldest", () => {
    const q = createQueue(2);
    q.add("/a.png");
    q.add("/b.png");
    q.add("/c.png");
    expect(q.items.map((i) => i.path)).toEqual(["/c.png", "/b.png"]);
  });

  it("dismiss removes by path", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    q.dismiss("/a.png");
    expect(q.items.map((i) => i.path)).toEqual(["/b.png"]);
  });

  it("touch bumps version for cache-busting after edits", () => {
    const q = createQueue(5);
    q.add("/a.png");
    const before = q.items[0].version;
    q.touch("/a.png");
    expect(q.items[0].version).toBe(before + 1);
  });

  it("re-adding an existing path moves it to front instead of duplicating", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    q.add("/a.png");
    expect(q.items.map((i) => i.path)).toEqual(["/a.png", "/b.png"]);
  });
});
```

- [ ] **Step 2: Run `npm run test` — expect 5 failures (module missing).**

- [ ] **Step 3: Implement `src/overlay/queue.ts`**

```ts
import { reactive } from "vue";

export interface CaptureItem {
  path: string;
  version: number;
}

export function createQueue(max: number) {
  const items = reactive<CaptureItem[]>([]);

  function add(path: string) {
    const existing = items.findIndex((i) => i.path === path);
    if (existing !== -1) items.splice(existing, 1);
    items.unshift({ path, version: 0 });
    if (items.length > max) items.length = max;
  }

  function dismiss(path: string) {
    const i = items.findIndex((it) => it.path === path);
    if (i !== -1) items.splice(i, 1);
  }

  function touch(path: string) {
    const item = items.find((it) => it.path === path);
    if (item) item.version++;
  }

  return { items, add, dismiss, touch };
}
```

- [ ] **Step 4: `npm run test` — expect 5 passed.**

- [ ] **Step 5: Implement `src/overlay/OverlayApp.vue`**

Layout: transparent page, thumbnails stacked vertically, newest at the BOTTOM (CleanShot behavior: new capture slides in at the bottom corner). The queue is newest-first, so render `[...queue.items].reverse()`. Each card: 224×140, rounded-12, shadow, screenshot via `convertFileSrc(path) + "?v=" + version`, object-fit cover. Hover reveals an action bar: ✏️ Edit · ⧉ Copy · 🔍 Finder · ✕ Dismiss (use compact text/SVG buttons, not emoji, styled dark translucent).

```vue
<template>
  <div class="stack">
    <div
      v-for="item in displayItems"
      :key="item.path"
      class="card"
      @click="edit(item)"
    >
      <img :src="src(item)" alt="" draggable="false" />
      <div class="actions" @click.stop>
        <button title="Edit" @click="edit(item)">Edit</button>
        <button title="Copy" @click="copy(item)">Copy</button>
        <button title="Show in Finder" @click="reveal(item)">Finder</button>
        <button title="Dismiss" class="dismiss" @click="dismiss(item)">✕</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, watch } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createQueue, type CaptureItem } from "./queue";

const CARD_W = 224;
const CARD_H = 140;
const GAP = 10;
const PAD = 8; // room for shadows

const queue = createQueue(5);
const displayItems = computed(() => [...queue.items].reverse());

function src(item: CaptureItem) {
  return convertFileSrc(item.path) + "?v=" + item.version;
}

async function syncWindow() {
  await nextTick();
  const n = queue.items.length;
  if (n === 0) {
    await invoke("hide_overlay");
    return;
  }
  const height = n * CARD_H + (n - 1) * GAP + PAD * 2;
  await invoke("resize_overlay", { width: CARD_W + PAD * 2, height });
  await invoke("show_overlay");
}

watch(() => queue.items.length, syncWindow);

function edit(item: CaptureItem) {
  invoke("open_editor", { path: item.path });
}
function copy(item: CaptureItem) {
  invoke("copy_image", { path: item.path });
}
function reveal(item: CaptureItem) {
  invoke("reveal_in_finder", { path: item.path });
}
function dismiss(item: CaptureItem) {
  queue.dismiss(item.path);
}

onMounted(async () => {
  await listen<{ path: string }>("capture-taken", (e) => {
    queue.add(e.payload.path);
    syncWindow(); // also covers version-only changes
  });
  await listen<{ path: string }>("capture-updated", (e) => {
    queue.touch(e.payload.path);
  });
});
</script>

<style scoped>
.stack {
  position: fixed;
  inset: 0;
  padding: 8px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 10px;
}
.card {
  position: relative;
  width: 224px;
  height: 140px;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 4px 18px rgba(0, 0, 0, 0.45);
  background: #1c1c1e;
  cursor: pointer;
  border: 1px solid rgba(255, 255, 255, 0.18);
}
.card img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.actions {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  gap: 4px;
  padding: 6px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.75));
  opacity: 0;
  transition: opacity 120ms ease;
}
.card:hover .actions {
  opacity: 1;
}
.actions button {
  flex: 1;
  font-size: 11px;
  padding: 4px 0;
  border: none;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.16);
  color: #fff;
  cursor: pointer;
}
.actions button:hover {
  background: rgba(255, 255, 255, 0.3);
}
.actions .dismiss {
  flex: 0 0 28px;
}
</style>
```

Global style requirement: the overlay page background must be fully transparent. In `src/App.vue`:

```vue
<template>
  <OverlayApp />
</template>

<script setup lang="ts">
import OverlayApp from "./overlay/OverlayApp.vue";
</script>

<style>
html, body, #app { margin: 0; background: transparent !important; }
</style>
```

Remove any template CSS (`src/assets`, default `style.css` import in `src/main.ts`) that sets a background color.

- [ ] **Step 6: Manual verification**

`npm run tauri dev`:
1. Take a capture (tray or hotkey) → thumbnail appears bottom-left, floating, transparent background, no focus stolen from the active app.
2. Take 2 more → they stack; newest at bottom.
3. Hover → actions appear. Copy → paste works. Finder → reveals file. Dismiss → card disappears; dismissing all hides the overlay entirely (no ghost window blocking clicks).
4. Open a fullscreen app (e.g. fullscreen Safari), take a capture via hotkey → thumbnail appears over the fullscreen app.
5. Click a thumbnail → editor window opens (placeholder page for now).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: overlay thumbnail queue with edit/copy/finder/dismiss actions"
```

---

### Task 6: Editor pure logic — history + geometry (TDD)

**Files:**
- Create: `src/editor/history.ts`, `src/editor/history.test.ts`, `src/editor/geometry.ts`, `src/editor/geometry.test.ts`

- [ ] **Step 1: Write failing tests `src/editor/history.test.ts`**

```ts
import { describe, it, expect } from "vitest";
import { History } from "./history";

describe("History", () => {
  it("undo returns previous state, redo returns it back", () => {
    const h = new History<string>("a");
    h.push("b");
    h.push("c");
    expect(h.undo()).toBe("b");
    expect(h.undo()).toBe("a");
    expect(h.redo()).toBe("b");
    expect(h.current).toBe("b");
  });

  it("push clears the redo stack", () => {
    const h = new History<string>("a");
    h.push("b");
    h.undo();
    h.push("c");
    expect(h.canRedo).toBe(false);
    expect(h.current).toBe("c");
  });

  it("undo/redo at the boundaries are no-ops", () => {
    const h = new History<string>("a");
    expect(h.canUndo).toBe(false);
    expect(h.undo()).toBe("a");
    expect(h.redo()).toBe("a");
  });
});
```

- [ ] **Step 2: Failing tests `src/editor/geometry.test.ts`**

```ts
import { describe, it, expect } from "vitest";
import { normalizeRect, clampRect, fitScale, aspectResize } from "./geometry";

describe("normalizeRect", () => {
  it("handles drags in any direction", () => {
    expect(normalizeRect({ x: 10, y: 10 }, { x: 4, y: 2 })).toEqual({
      x: 4, y: 2, width: 6, height: 8,
    });
  });
});

describe("clampRect", () => {
  it("clips a rect to image bounds", () => {
    expect(clampRect({ x: -5, y: 10, width: 30, height: 200 }, 100, 100)).toEqual({
      x: 0, y: 10, width: 25, height: 90,
    });
  });
  it("returns null when fully outside or degenerate", () => {
    expect(clampRect({ x: 200, y: 0, width: 10, height: 10 }, 100, 100)).toBeNull();
    expect(clampRect({ x: 0, y: 0, width: 0, height: 5 }, 100, 100)).toBeNull();
  });
});

describe("fitScale", () => {
  it("scales down to fit, never up", () => {
    expect(fitScale(2000, 1000, 1000, 800)).toBe(0.5);
    expect(fitScale(400, 300, 1000, 800)).toBe(1);
  });
});

describe("aspectResize", () => {
  it("derives the other dimension from aspect ratio, rounded", () => {
    expect(aspectResize(1600, 900, { width: 800 })).toEqual({ width: 800, height: 450 });
    expect(aspectResize(1600, 900, { height: 450 })).toEqual({ width: 800, height: 450 });
  });
});
```

- [ ] **Step 3: Run `npm run test` — new files fail (modules missing); queue tests still pass.**

- [ ] **Step 4: Implement `src/editor/history.ts`**

```ts
export class History<T> {
  private past: T[] = [];
  private future: T[] = [];
  private present: T;

  constructor(initial: T) {
    this.present = initial;
  }

  get current(): T {
    return this.present;
  }
  get canUndo(): boolean {
    return this.past.length > 0;
  }
  get canRedo(): boolean {
    return this.future.length > 0;
  }

  push(next: T): void {
    this.past.push(this.present);
    this.present = next;
    this.future = [];
  }

  undo(): T {
    const prev = this.past.pop();
    if (prev !== undefined) {
      this.future.push(this.present);
      this.present = prev;
    }
    return this.present;
  }

  redo(): T {
    const next = this.future.pop();
    if (next !== undefined) {
      this.past.push(this.present);
      this.present = next;
    }
    return this.present;
  }
}
```

- [ ] **Step 5: Implement `src/editor/geometry.ts`**

```ts
export interface Point { x: number; y: number }
export interface Rect { x: number; y: number; width: number; height: number }

export function normalizeRect(a: Point, b: Point): Rect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(b.x - a.x),
    height: Math.abs(b.y - a.y),
  };
}

export function clampRect(r: Rect, imgW: number, imgH: number): Rect | null {
  const x = Math.max(0, r.x);
  const y = Math.max(0, r.y);
  const width = Math.min(r.x + r.width, imgW) - x;
  const height = Math.min(r.y + r.height, imgH) - y;
  if (width <= 0 || height <= 0) return null;
  return { x, y, width, height };
}

export function fitScale(imgW: number, imgH: number, maxW: number, maxH: number): number {
  return Math.min(1, maxW / imgW, maxH / imgH);
}

export function aspectResize(
  origW: number,
  origH: number,
  target: { width?: number; height?: number },
): { width: number; height: number } {
  if (target.width !== undefined) {
    return { width: target.width, height: Math.round((target.width * origH) / origW) };
  }
  if (target.height !== undefined) {
    return { width: Math.round((target.height * origW) / origH), height: target.height };
  }
  return { width: origW, height: origH };
}
```

- [ ] **Step 6: `npm run test` — all pass. Commit**

```bash
git add -A
git commit -m "feat: editor history stack and geometry utilities with tests"
```

---

### Task 7: Editor shell — load image, scaled stage, toolbar, select tool

**Files:**
- Create: `src/editor/EditorApp.vue` (replace placeholder), `src/editor/store.ts`
- Modify: `src/editor/main.ts` (no change expected; verify it registers VueKonva)

**Editor state model (used by Tasks 7–9):** `src/editor/store.ts`

- [ ] **Step 1: Create `src/editor/store.ts`**

```ts
import { reactive } from "vue";
import { History } from "./history";

export type Tool = "select" | "rect" | "blur" | "crop";

export interface RectShape {
  kind: "rect";
  id: string;
  x: number; y: number; width: number; height: number;
  stroke: string;
  strokeWidth: number;
}

export interface BlurShape {
  kind: "blur";
  id: string;
  // region in image pixel coordinates
  x: number; y: number; width: number; height: number;
  pixelSize: number;
}

export type Shape = RectShape | BlurShape;

export interface EditorSnapshot {
  /** data URL or asset URL of the base bitmap */
  baseSrc: string;
  baseWidth: number;
  baseHeight: number;
  shapes: Shape[];
}

export function cloneSnapshot(s: EditorSnapshot): EditorSnapshot {
  return JSON.parse(JSON.stringify(s));
}

let nextId = 1;
export function shapeId(): string {
  return `s${nextId++}`;
}

export function createEditorState() {
  return reactive({
    tool: "select" as Tool,
    filePath: "",
    snapshot: null as EditorSnapshot | null,
    history: null as History<EditorSnapshot> | null,
    selectedId: "" as string,
    stroke: "#ff3b30",
    strokeWidth: 4,
    error: "" as string,
  });
}
```

- [ ] **Step 2: Implement `src/editor/EditorApp.vue`**

Responsibilities in this task (tools come next task):
- Resolve file path: from `new URLSearchParams(location.search).get("path")` AND from a `getCurrentWebviewWindow().listen("editor-load", ...)` event (replaces current content; confirm-free — saving is explicit).
- Load the bitmap: `convertFileSrc(path)` → `new Image()` → onload → seed `snapshot` (`baseSrc` = asset URL, natural dims, `shapes: []`) and `history = new History(cloneSnapshot(snapshot))`.
- Compute display scale via `fitScale(baseW, baseH, viewportW - 32, viewportH - toolbarH - 32)`; re-compute on window resize.
- Render Konva stage: `width: baseW*scale, height: baseH*scale, scaleX: scale, scaleY: scale`. Layer 1: `v-image` with the base bitmap at (0,0) natural size. Layer 2: shapes (empty for now) + `v-transformer`.
- Toolbar (top bar, dark): tool buttons Select / Rectangle / Blur / Crop, color swatches (red `#ff3b30`, blue `#0a84ff`, green `#30d158`, yellow `#ffd60a`, black `#000`), stroke width (2/4/6), Undo / Redo, spacer, Resize…, Copy, Save, Save As. Buttons disabled when no image. Active tool highlighted.
- Keyboard: `Cmd+Z` undo, `Shift+Cmd+Z` redo, `Delete`/`Backspace` delete selected shape, `Esc` → deselect (if selection) else close window (`getCurrentWindow().close()`).
- Error state: if image fails to load, show centered message "Could not load capture — the file may have been moved or deleted." with a Close button.

Skeleton (the implementing agent fills in straightforward wiring, keeping ALL function/property names exactly as below — later tasks reference them):

```vue
<template>
  <div class="editor">
    <div class="toolbar">
      <button v-for="t in tools" :key="t.id"
        :class="{ active: state.tool === t.id }"
        @click="state.tool = t.id">{{ t.label }}</button>
      <span class="sep" />
      <button v-for="c in colors" :key="c" class="swatch"
        :style="{ background: c }"
        :class="{ active: state.stroke === c }"
        @click="state.stroke = c" />
      <select v-model.number="state.strokeWidth">
        <option :value="2">2 px</option><option :value="4">4 px</option><option :value="6">6 px</option>
      </select>
      <span class="sep" />
      <button :disabled="!canUndo" @click="undo">Undo</button>
      <button :disabled="!canRedo" @click="redo">Redo</button>
      <span class="spacer" />
      <button @click="openResize">Resize…</button>
      <button @click="copyResult">Copy</button>
      <button @click="saveResult">Save</button>
      <button @click="saveAsResult">Save As…</button>
    </div>
    <div ref="viewport" class="viewport">
      <div v-if="state.error" class="error">{{ state.error }}</div>
      <v-stage v-else-if="snapshot" ref="stageRef" :config="stageConfig"
        @mousedown="onMouseDown" @mousemove="onMouseMove" @mouseup="onMouseUp">
        <v-layer>
          <v-image v-if="baseImageEl" :config="{ image: baseImageEl, x: 0, y: 0 }" />
        </v-layer>
        <v-layer ref="shapesLayerRef">
          <!-- shapes rendered in Task 8 -->
          <v-transformer ref="transformerRef" :config="{ rotateEnabled: false }" />
        </v-layer>
      </v-stage>
    </div>
  </div>
</template>
```

Key implementation notes for the agent:
- `stageConfig` = `{ width: snap.baseWidth * scale, height: snap.baseHeight * scale, scaleX: scale, scaleY: scale }`.
- Pointer position in IMAGE coordinates: `const pos = stageRef.value.getNode().getPointerPosition(); const p = { x: pos.x / scale, y: pos.y / scale }`. Wrap as `pointerInImage()` helper — Tasks 8–9 use it.
- `commit()` helper: `state.history.push(cloneSnapshot(state.snapshot))` BEFORE applying a mutation is wrong — the convention is: mutate `state.snapshot`, then `commit()` pushes the new clone. Implement as: `function commit() { history.push(cloneSnapshot(state.snapshot)) }` where `history` was seeded with the initial snapshot; `undo()`/`redo()` set `state.snapshot = cloneSnapshot(history.undo()/redo())`.
- Loading from the `editor-load` event resets snapshot, history, selection, and error.
- `onMouseDown/Move/Up` dispatch on `state.tool`; in this task only `select` exists (click shape → `state.selectedId`, click empty → clear). Transformer attach: watch `selectedId`, `transformerRef.value.getNode().nodes(node ? [node] : [])` via `stage.findOne("#" + id)` (shapes will set Konva `id`).
- `copyResult`/`saveResult`/`saveAsResult`/`openResize` are stubs this task (`console.warn("not implemented")`); wired in Tasks 9–10.

Dark UI: toolbar `#2c2c2e`, viewport `#1c1c1e`, buttons translucent white like the overlay.

- [ ] **Step 3: Manual verification**

`npm run tauri dev`, take a capture, click thumbnail:
1. Editor opens showing the screenshot scaled to fit; window resize re-fits.
2. A second capture + Edit replaces the content in the same editor window.
3. Deleting the file in Finder then opening it from a stale thumbnail shows the error state.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: editor shell — image loading, scaled stage, toolbar, selection"
```

---

### Task 8: Rectangle + blur tools

**Files:**
- Create: `src/editor/BlurPatch.vue`
- Modify: `src/editor/EditorApp.vue`

- [ ] **Step 1: Rect tool**

In `EditorApp.vue`, implement drag-to-draw using `pointerInImage()` and `normalizeRect` from geometry:

- `onMouseDown` (tool === "rect"): record `dragStart`, set `drafting = true`.
- `onMouseMove`: update `dragCurrent`; a computed `draftRect = normalizeRect(dragStart, dragCurrent)` renders as a live `v-rect` (stroke `state.stroke`, strokeWidth `state.strokeWidth`, dash `[6,4]` while drafting).
- `onMouseUp`: if `draftRect.width < 4 || height < 4` discard; else push into `state.snapshot.shapes`:

```ts
state.snapshot.shapes.push({
  kind: "rect",
  id: shapeId(),
  ...draftRect,
  stroke: state.stroke,
  strokeWidth: state.strokeWidth,
});
commit();
state.tool = "select";
state.selectedId = newId; // select the fresh rect
```

- Render committed rects:

```vue
<v-rect v-for="s in rectShapes" :key="s.id"
  :config="{ id: s.id, x: s.x, y: s.y, width: s.width, height: s.height,
             stroke: s.stroke, strokeWidth: s.strokeWidth, draggable: state.tool === 'select',
             strokeScaleEnabled: true }"
  @dragend="onShapeDragEnd(s, $event)"
  @transformend="onShapeTransformEnd(s, $event)" />
```

- `onShapeDragEnd`: write `e.target.x()/y()` back to the shape, `commit()`.
- `onShapeTransformEnd`: bake scale into size, reset scale:

```ts
const node = e.target;
s.x = node.x(); s.y = node.y();
s.width = Math.max(4, node.width() * node.scaleX());
s.height = Math.max(4, node.height() * node.scaleY());
node.scaleX(1); node.scaleY(1);
commit();
```

- `Delete` key: remove shape with `selectedId` from `snapshot.shapes`, clear selection, `commit()`.

- [ ] **Step 2: Blur tool — `src/editor/BlurPatch.vue`**

A blur region is a `v-image` showing the base bitmap cropped to the region, pixelated. Konva filters require `cache()` after mount and after any config change:

```vue
<template>
  <v-image ref="nodeRef" :config="config" />
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch, nextTick } from "vue";
import Konva from "konva";
import type { BlurShape } from "./store";

const props = defineProps<{ shape: BlurShape; image: HTMLImageElement }>();
const nodeRef = ref();

const config = computed(() => ({
  id: props.shape.id,
  x: props.shape.x,
  y: props.shape.y,
  width: props.shape.width,
  height: props.shape.height,
  image: props.image,
  crop: {
    x: props.shape.x,
    y: props.shape.y,
    width: props.shape.width,
    height: props.shape.height,
  },
  filters: [Konva.Filters.Pixelate],
  pixelSize: props.shape.pixelSize,
  listening: true,
  draggable: false,
}));

function recache() {
  const node: Konva.Image = nodeRef.value?.getNode();
  if (node) {
    node.cache();
    node.getLayer()?.batchDraw();
  }
}

onMounted(async () => {
  await nextTick();
  recache();
});
watch(config, async () => {
  await nextTick();
  recache();
});
</script>
```

- [ ] **Step 3: Blur drawing in `EditorApp.vue`**

Same drag pattern as rect (live draft shown as a semi-transparent gray `v-rect`). On mouseup, `clampRect(draftRect, baseWidth, baseHeight)` (blur crop must stay inside the bitmap — `crop` outside bounds renders garbage); if null, discard. Else:

```ts
state.snapshot.shapes.push({
  kind: "blur",
  id: shapeId(),
  ...clamped,
  pixelSize: Math.max(8, Math.round(Math.min(clamped.width, clamped.height) / 12)),
});
commit();
state.tool = "select";
```

Render before rect shapes (blur under annotation rects):

```vue
<BlurPatch v-for="s in blurShapes" :key="s.id" :shape="s" :image="baseImageEl" />
```

Blur regions in phase 1 are NOT draggable/resizable (re-cropping a cached filtered node correctly is post-phase-1); they ARE selectable (click → `selectedId`) and deletable via `Delete`. The transformer must not attach resize handles to blur shapes: when the selected node is a blur shape, call `.nodes([node])` with `resizeEnabled: false` set on the transformer, or simpler — attach the transformer only for rect shapes and indicate blur selection with a 1px dashed `v-rect` outline drawn over the blur region.

(Simplest correct choice: transformer only for rects; blur selection outline as dashed rect. Implement that.)

- [ ] **Step 4: Undo/redo verification of both tools**

Manual: draw rect → move it → blur a region → Cmd+Z ×3 steps back cleanly → redo restores. Blur patch must re-pixelate after undo/redo (the `watch` + `recache` handles re-mounts via `:key`).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: rectangle and pixelate-blur annotation tools with undo/redo"
```

---

### Task 9: Crop + resize (flatten-and-replace)

**Files:**
- Modify: `src/editor/EditorApp.vue`
- Create: `src/editor/flatten.ts`

- [ ] **Step 1: Create `src/editor/flatten.ts`**

```ts
import type Konva from "konva";

/**
 * Flattens the stage to a canvas at FULL image resolution.
 * The stage is displayed at `scale`; pixelRatio 1/scale recovers 1:1 pixels.
 * Detaches the transformer during the snapshot so handles never bake in.
 */
export async function flattenStage(
  stage: Konva.Stage,
  transformer: Konva.Transformer,
  scale: number,
): Promise<HTMLCanvasElement> {
  const prevNodes = transformer.nodes();
  transformer.nodes([]);
  await new Promise((r) => setTimeout(r, 50)); // one render cycle
  const canvas = stage.toCanvas({ pixelRatio: 1 / scale });
  transformer.nodes(prevNodes);
  return canvas;
}

export function cropCanvas(src: HTMLCanvasElement, r: { x: number; y: number; width: number; height: number }): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = Math.round(r.width);
  out.height = Math.round(r.height);
  out.getContext("2d")!.drawImage(src, r.x, r.y, r.width, r.height, 0, 0, out.width, out.height);
  return out;
}

export function scaleCanvas(src: HTMLCanvasElement, w: number, h: number): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = w;
  out.height = h;
  const ctx = out.getContext("2d")!;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(src, 0, 0, w, h);
  return out;
}
```

- [ ] **Step 2: Crop tool in `EditorApp.vue`**

- Drag pattern again: draft shown as dashed rect + dimmed surroundings (4 semi-transparent black `v-rect`s around the draft, or skip dimming if fiddly — dashed rect is the requirement).
- On mouseup with a valid `clampRect`: show a small floating confirm bar near the rect ("Crop ✓ / ✕" buttons absolutely positioned over the viewport). On confirm:

```ts
async function applyCrop(region: Rect) {
  const canvas = await flattenStage(stage, transformer, scale.value);
  const out = cropCanvas(canvas, region);
  replaceBase(out); // shared helper below
}

function replaceBase(canvas: HTMLCanvasElement) {
  const dataUrl = canvas.toDataURL("image/png");
  state.snapshot.baseSrc = dataUrl;
  state.snapshot.baseWidth = canvas.width;
  state.snapshot.baseHeight = canvas.height;
  state.snapshot.shapes = []; // annotations are baked into the new base
  state.selectedId = "";
  commit();
  // reload baseImageEl from dataUrl (same loader as initial load)
}
```

Crop/resize FLATTEN annotations into the new base (CleanShot behaves the same way); undo restores the previous base + live shapes because snapshots store `baseSrc`.

- [ ] **Step 3: Resize dialog**

`openResize()`: minimal inline modal (no library): width/height number inputs pre-filled with current dims, lock-aspect checkbox (default on, uses `aspectResize` on input), Apply/Cancel. Apply:

```ts
async function applyResize(w: number, h: number) {
  const canvas = await flattenStage(stage, transformer, scale.value);
  replaceBase(scaleCanvas(canvas, w, h));
}
```

Validate: integers, min 8, max 4× original (reject with inline message otherwise).

- [ ] **Step 4: Undo of base replacement**

`undo()`/`redo()` already restore `baseSrc`; make the base-image loader watch `snapshot.baseSrc` and reload `baseImageEl` whenever it changes (data URL or asset URL both load through `new Image()`). Verify scale recomputes from restored dims.

- [ ] **Step 5: Manual verification**

1. Annotate → crop → annotations baked, canvas shrinks, stage re-fits.
2. Cmd+Z → original size AND live annotations return.
3. Resize 50% → dimensions halve; resize dialog shows new dims on reopen.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: crop and resize via flatten-and-replace with undo support"
```

---

### Task 10: Export — Copy, Save, Save As

**Files:**
- Modify: `src/editor/EditorApp.vue`

- [ ] **Step 1: Shared export helper in `EditorApp.vue`**

```ts
async function exportPngBase64(): Promise<string> {
  const canvas = await flattenStage(stage, transformer, scale.value);
  return canvas.toDataURL("image/png").split(",")[1];
}
```

- [ ] **Step 2: Wire the three actions**

```ts
import { save } from "@tauri-apps/plugin-dialog";

async function copyResult() {
  await invoke("copy_image_data", { base64Png: await exportPngBase64() });
  flash("Copied");
}

async function saveResult() {
  await invoke("save_png", { path: state.filePath, base64Png: await exportPngBase64() });
  flash("Saved");
}

async function saveAsResult() {
  const target = await save({
    defaultPath: state.filePath.replace(/\.png$/, " edited.png"),
    filters: [{ name: "PNG", extensions: ["png"] }],
  });
  if (!target) return;
  await invoke("save_png", { path: target, base64Png: await exportPngBase64() });
  flash("Saved");
}
```

Note: Tauri converts Rust snake_case args to camelCase on the JS side — `base64_png` → `base64Png`. `flash(msg)` = transient 1.5s toast in the toolbar area (simple span with timeout).

Add `"dialog:allow-save"` to capabilities if `dialog:default` doesn't already include it (check `npm run tauri dev` console for permission errors; `dialog:default` does include save — verify empirically).

- [ ] **Step 3: Manual verification**

1. Annotate → Copy → paste in Notes: annotations + blur baked, FULL resolution (Retina: pasted image pixel size equals original capture, not the scaled-down display).
2. Save → thumbnail in overlay refreshes to show annotations (capture-updated → `touch` → cache-busted `?v=`).
3. Save As → writes the new file; original untouched.
4. Blur check: zoom the pasted image — blurred region must be unreadable pixels, not a CSS-style soft blur over readable text.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: editor export — copy to clipboard, save, save as"
```

---

### Task 11: Polish + README + full smoke run

**Files:**
- Create: `README.md`
- Modify: anything surfaced by the smoke run

- [ ] **Step 1: Full smoke checklist (fix anything that fails before proceeding)**

1. `npm run test` — all green. `cd src-tauri && cargo test` — all green.
2. Cold start `npm run tauri dev`: menubar-only, no Dock icon, no windows shown.
3. Hotkey capture → Esc → nothing happens, no file.
4. Hotkey capture → drag region → clipboard paste OK, thumbnail appears without stealing focus from the frontmost app (verify: type in another app while capturing — focus stays there after the thumbnail appears).
5. Window capture (press Space in the native UI, click a window) → works the same.
6. 6 rapid captures → overlay shows 5, oldest dropped, files all on disk.
7. Full editor loop: rect → blur → crop → resize → undo×4 → redo×2 → Save → overlay thumbnail refreshes.
8. Quit via tray; relaunch; overlay empty (no persistence in phase 1 — expected).

- [ ] **Step 2: Write `README.md`**

Cover: what DigitShot is (personal CleanShot X-style tool), phase-1 features, hotkey `Cmd+Shift+2`, captures folder `~/Pictures/DigitShot/`, dev (`npm install`, `npm run tauri dev`), tests (`npm run test`, `cargo test`), build (`npm run tauri build`), architecture pointer to the spec/plan docs, known limitations (blur regions fixed once drawn, single editor window, no settings UI yet).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "docs: README and phase-1 smoke fixes"
```
