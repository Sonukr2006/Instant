use tauri::{Manager, PhysicalPosition, Position, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn toggle_overlay(app: tauri::AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();

    for window in windows.values() {
        if window.label() == "overlay" {
            let is_visible = window
                .is_visible()
                .map_err(|e| format!("Failed to check visibility: {}", e))?;

            eprintln!("Toggling overlay. Current visible state: {}", is_visible);

            if is_visible {
                window
                    .hide()
                    .map_err(|e| format!("Failed to hide window: {}", e))?;
                eprintln!("Overlay hidden");
            } else {
                window
                    .show()
                    .map_err(|e| format!("Failed to show window: {}", e))?;
                window
                    .set_focus()
                    .map_err(|e| format!("Failed to set focus: {}", e))?;
                eprintln!("Overlay shown and focused");

                if let Ok(Some(monitor)) = window.current_monitor() {
                    let x = (monitor.size().width as i32 - 600) / 2 + monitor.position().x;
                    let y = (monitor.size().height as i32 - 400) / 2 + monitor.position().y;

                    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                }
            }

            return Ok(());
        }
    }

    Err("Overlay window not found".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["ctrl+alt+k", "ctrl+shift+space"])
                .expect("failed to configure global shortcuts")
                .with_handler(|app, shortcut, event| {
                    eprintln!(
                        "Global shortcut event: {} {:?}",
                        shortcut.clone().into_string(),
                        event.state
                    );

                    if event.state == ShortcutState::Pressed {
                        if let Err(e) = toggle_overlay(app.clone()) {
                            eprintln!("Error toggling overlay: {}", e);
                        }
                    }
                })
                .build(),
        )
        .setup(|_app| {
            #[cfg(desktop)]
            {
                eprintln!("Registered global shortcuts: ctrl+alt+k, ctrl+shift+space");
            }

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![greet, toggle_overlay])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
