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
