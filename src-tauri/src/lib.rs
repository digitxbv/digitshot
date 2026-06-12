mod capture;
mod clipboard;

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
    OverlayPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            capture_interactive,
            copy_image,
            copy_image_data,
            save_png,
            reveal_in_finder,
            open_editor,
            resize_overlay,
            show_overlay,
            hide_overlay,
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
