mod capture;
mod clipboard;
mod permissions;
mod scrolling;
mod stitch;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
    panel!(SelectorPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
    panel!(FramePanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

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

/// Raw PNG bytes for the editor. The webview must NOT load editable images via
/// the asset protocol: WKWebView taints canvases for custom-scheme images
/// (WebKit bug 201180), breaking toDataURL/getImageData.
#[tauri::command]
fn read_image(path: String) -> Result<tauri::ipc::Response, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
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
            .title(if cfg!(debug_assertions) { "DigitShot Dev" } else { "DigitShot" })
            .inner_size(1160.0, 800.0)
            .build()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

const OVERLAY_MARGIN: i32 = 16;

/// Resize the overlay to fit its content and pin it bottom-left of the
/// primary monitor's work area.
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

#[tauri::command]
fn filter_existing(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

/// Cover the primary monitor with the click-through frame window, tell its
/// page where the region is, and show it without focus.
fn show_frame(app: &AppHandle, region: &scrolling::Region) {
    let win = match app.get_webview_window("frame") {
        Some(w) => w,
        None => {
            eprintln!("[frame] no 'frame' window!");
            return;
        }
    };
    match win.primary_monitor() {
        Ok(Some(monitor)) => {
            let scale = monitor.scale_factor();
            let _ = win.set_size(tauri::LogicalSize::new(
                monitor.size().width as f64 / scale,
                monitor.size().height as f64 / scale,
            ));
            let _ = win.set_position(tauri::LogicalPosition::new(0.0, 0.0));
        }
        _ => {
            eprintln!("[frame] no primary monitor");
        }
    }
    let _ = win.set_ignore_cursor_events(true); // clicks pass through to the app below
    let _ = win.emit("scroll-region", region.clone());
    eprintln!("[frame] region emitted: {:?}", region);
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        match app.get_webview_panel("frame") {
            Ok(panel) => {
                panel.show();
                eprintln!("[frame] panel shown");
            }
            Err(e) => {
                eprintln!("[frame] get_webview_panel failed: {e:?}");
                if let Some(w) = app.get_webview_window("frame") {
                    let _ = w.show();
                    eprintln!("[frame] fell back to window.show()");
                }
            }
        }
    }
}

fn hide_frame(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        match app.get_webview_panel("frame") {
            Ok(panel) => panel.hide(),
            Err(e) => eprintln!("[frame] hide: get_webview_panel failed: {e:?}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("frame") {
        let _ = w.hide();
    }
}

/// Entry point for tray/hotkey/selector. Gates on the Screen Recording grant.
fn begin_scroll_capture(app: &AppHandle) {
    // A session is already running: ignore re-triggers instead of overlaying
    // the fullscreen selector on top of an active capture.
    if let Some(state) = app.try_state::<scrolling::ScrollState>() {
        if state.0.lock().unwrap().is_some() {
            return;
        }
    }
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
                panel.show_and_make_key();
            }
        }
    }
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
    *guard = Some(scrolling::start(app.clone(), region)?);
    show_frame(&app, &region);
    Ok(())
}

#[tauri::command]
fn scroll_capture_stop(
    app: AppHandle,
    state: tauri::State<scrolling::ScrollState>,
) -> Result<(), String> {
    let session = state.0.lock().unwrap().take().ok_or("no session")?;
    hide_selector(&app);
    hide_frame(&app);
    let stitched = match scrolling::stop(session) {
        Ok(s) => s,
        Err(e) => {
            let _ = app.emit("scroll-capture-error", e.clone());
            return Err(e);
        }
    };
    if let Some(stitched) = stitched {
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
    hide_frame(&app);
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
    const PANEL_W: f64 = 440.0;
    const PANEL_H: f64 = 64.0;
    let rx = region.x as f64;
    let ry = region.y as f64;
    let rw = region.width as f64;
    let rh = region.height as f64;
    const GAP: f64 = 12.0;
    // Preferred placements, in order: below, above, right side, left side.
    // Each must fit on the monitor; sides use the region's vertical middle.
    let (x, y) = if ry + rh + GAP + PANEL_H <= mon_h {
        ((rx + rw / 2.0 - PANEL_W / 2.0).clamp(8.0, mon_w - PANEL_W - 8.0), ry + rh + GAP)
    } else if ry - GAP - PANEL_H >= 0.0 {
        ((rx + rw / 2.0 - PANEL_W / 2.0).clamp(8.0, mon_w - PANEL_W - 8.0), ry - GAP - PANEL_H)
    } else if rx + rw + GAP + PANEL_W <= mon_w {
        (rx + rw + GAP, (ry + rh / 2.0 - PANEL_H / 2.0).clamp(8.0, mon_h - PANEL_H - 8.0))
    } else if rx - GAP - PANEL_W >= 0.0 {
        (rx - GAP - PANEL_W, (ry + rh / 2.0 - PANEL_H / 2.0).clamp(8.0, mon_h - PANEL_H - 8.0))
    } else {
        // Region covers essentially the whole screen; top-center overlap is the
        // least-bad option (better than blocking the content being scrolled).
        ((mon_w / 2.0 - PANEL_W / 2.0).max(8.0), 8.0)
    };
    win.set_size(tauri::LogicalSize::new(PANEL_W, PANEL_H)).map_err(|e| e.to_string())?;
    win.set_position(tauri::LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(scrolling::ScrollState::default())
        .invoke_handler(tauri::generate_handler![
            capture_interactive,
            copy_image,
            copy_image_data,
            save_png,
            read_image,
            reveal_in_finder,
            open_editor,
            resize_overlay,
            show_overlay,
            hide_overlay,
            scroll_capture_begin,
            scroll_capture_start,
            scroll_capture_stop,
            scroll_capture_cancel,
            position_scroll_panel,
            filter_existing,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Global shortcuts:
            //   Capture area:        Cmd+Shift+2 (release) / Cmd+Shift+1 (dev)
            //   Scrolling capture:   Cmd+Shift+6 (release) / Cmd+Shift+0 (dev)
            // Dev builds use distinct keys so a dev instance can run alongside the
            // installed release app without fighting over the hotkeys.
            {
                use tauri_plugin_global_shortcut::{
                    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
                };
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
                let capture_shortcut_handler = capture_shortcut.clone();
                let scroll_shortcut_handler = scroll_shortcut.clone();
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(move |app, sc, event| {
                            if event.state() != ShortcutState::Pressed {
                                return;
                            }
                            if sc == &capture_shortcut_handler {
                                trigger_capture(app);
                            } else if sc == &scroll_shortcut_handler {
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
            }

            // Tray
            let capture_item = MenuItem::with_id(app, "capture", "Capture Area", true, None::<&str>)?;
            let scroll_item = MenuItem::with_id(app, "scroll", "Scrolling Capture", true, None::<&str>)?;
            let folder_item =
                MenuItem::with_id(app, "folder", "Open Captures Folder", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit DigitShot", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&capture_item, &scroll_item, &folder_item, &quit_item])?;
            let mut tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "capture" => trigger_capture(app),
                    "scroll" => begin_scroll_capture(app),
                    "folder" => {
                        use tauri_plugin_opener::OpenerExt;
                        let dir = capture::captures_dir(&home_dir());
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            #[cfg(target_os = "macos")]
            if cfg!(debug_assertions) {
                tray = tray.title("DEV");
            }
            tray.build(app)?;

            // NSPanel overlay setup
            #[cfg(target_os = "macos")]
            {
                let window = app.get_webview_window("overlay").unwrap();
                let panel = window.to_panel::<OverlayPanel>().unwrap();
                panel.set_level(PanelLevel::Status.value());
                panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());
                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .can_join_all_spaces()
                        .full_screen_auxiliary()
                        .value(),
                );
                panel.set_hides_on_deactivate(false);
            }

            // NSPanel selector setup
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

            // NSPanel frame setup
            #[cfg(target_os = "macos")]
            {
                let window = app.get_webview_window("frame").unwrap();
                let panel = window.to_panel::<FramePanel>().unwrap();
                panel.set_level(PanelLevel::Status.value());
                panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());
                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .can_join_all_spaces()
                        .full_screen_auxiliary()
                        .value(),
                );
                panel.set_hides_on_deactivate(false);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // Menubar app: hiding/closing every window must NOT quit the
                // app. Only an explicit exit (tray Quit -> app.exit(0), code
                // Some) may pass.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
