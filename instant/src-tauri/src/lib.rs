use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, Position, WebviewWindow, WindowEvent,
};

const OVERLAY_WINDOW_LABEL: &str = "overlay";
const TRAY_ID: &str = "instant-tray";
const TRAY_TOGGLE_ID: &str = "toggle-overlay";
const TRAY_QUIT_ID: &str = "quit-app";
const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_GEMINI_API_VERSION: &str = "v1beta";
const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
const GEMINI_TIMEOUT_SECS: u64 = 35;
const MAX_PROMPT_CHARS: usize = 60_000;
const MAX_ERROR_DETAIL_CHARS: usize = 1_500;
const AI_CONFIG_FILE_NAME: &str = "instant-ai-context.json";
const AI_SYSTEM_PROMPT: &str = "You are a smart, adaptive developer assistant.
Analyze the incoming user payload before choosing a response style.
If the payload is a casual message, greeting, reading note, PDF excerpt, or general conceptual question, respond naturally, conversationally, and concisely as a helpful peer.
Only if the payload contains an explicit code snippet, structural data configuration, command output, or software error stack trace, activate a strict, professional diagnostic format.
In diagnostic mode, prioritize bugs, risks, correctness, security, performance, maintainability, and developer experience.
When refactoring is useful, provide optimized code with concise rationale and avoid vague advice.";

#[derive(serde::Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiRequestContent>,
}

#[derive(serde::Serialize)]
struct GeminiRequestContent {
    parts: Vec<GeminiRequestPart>,
}

#[derive(serde::Serialize)]
struct GeminiRequestPart {
    text: String,
}

#[derive(serde::Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(serde::Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(serde::Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(serde::Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[derive(serde::Deserialize)]
struct AiConfigFile {
    gemini_api_key: Option<String>,
    gemini_api_version: Option<String>,
    gemini_model: Option<String>,
}

struct AiConfig {
    api_key: String,
    api_version: String,
    model: String,
}

struct AiState {
    client: reqwest::Client,
}

#[tauri::command]
fn get_clipboard_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("Unable to access clipboard: {error}"))?;

    let text = clipboard
        .get_text()
        .map_err(|error| format!("Clipboard is empty or does not contain plain text: {error}"))?;

    if text.is_empty() {
        Err("Clipboard does not contain any text.".to_string())
    } else {
        Ok(text)
    }
}

#[tauri::command]
async fn fetch_ai_response(
    prompt_context: String,
    app: AppHandle,
    state: tauri::State<'_, AiState>,
) -> Result<String, String> {
    let prompt_context = prompt_context.trim();

    if prompt_context.is_empty() {
        return Err("Cannot request an AI response without text context.".to_string());
    }

    let prompt_char_count = prompt_context.chars().count();
    if prompt_char_count > MAX_PROMPT_CHARS {
        return Err(format!(
            "Context is too large to send safely. Please reduce it to {MAX_PROMPT_CHARS} characters or less. Current size: {prompt_char_count} characters."
        ));
    }

    let ai_config = resolve_ai_config(&app)?;
    let model = ai_config.model.trim_start_matches("models/");

    let payload = GeminiRequest {
        contents: vec![GeminiRequestContent {
            parts: vec![GeminiRequestPart {
                text: format!("{AI_SYSTEM_PROMPT}\n\nContext:\n{prompt_context}"),
            }],
        }],
    };

    let response = state
        .client
        .post(format!(
            "{GEMINI_API_BASE_URL}/{}/models/{model}:generateContent",
            ai_config.api_version
        ))
        .query(&[("key", ai_config.api_key.as_str())])
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Failed to reach Gemini API: {error}"))?;

    let status = response.status();

    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        let detail = truncate_error_detail(detail.trim());
        let suffix = if detail.is_empty() {
            String::new()
        } else {
            format!(" {detail}")
        };

        return Err(format!(
            "Gemini API request failed with HTTP {}.{}",
            status.as_u16(),
            suffix
        ));
    }

    let data = response
        .json::<GeminiResponse>()
        .await
        .map_err(|error| format!("Failed to parse Gemini response: {error}"))?;

    data.candidates
        .and_then(|candidates| candidates.into_iter().next())
        .and_then(|candidate| candidate.content)
        .and_then(|content| content.parts)
        .and_then(|parts| parts.into_iter().next())
        .and_then(|part| part.text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "Gemini response did not contain generated text.".to_string())
}

fn resolve_ai_config(app: &AppHandle) -> Result<AiConfig, String> {
    let file_config = read_ai_config_file(app);
    let config_path = ai_config_file_path(app);

    let api_key = read_optional_env("GEMINI_API_KEY")
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| clean_optional(&config.gemini_api_key))
        })
        .ok_or_else(|| {
            let file_hint = config_path
                .as_ref()
                .map(|path| format!(" or add gemini_api_key to {}", path.display()))
                .unwrap_or_default();

            format!("Missing Gemini API key. Set GEMINI_API_KEY{file_hint}.")
        })?;

    let api_version = read_optional_env("GEMINI_API_VERSION")
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| clean_optional(&config.gemini_api_version))
        })
        .unwrap_or_else(|| DEFAULT_GEMINI_API_VERSION.to_string());

    let model = read_optional_env("GEMINI_MODEL")
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| clean_optional(&config.gemini_model))
        })
        .unwrap_or_else(|| DEFAULT_GEMINI_MODEL.to_string());

    Ok(AiConfig {
        api_key,
        api_version,
        model,
    })
}

fn read_ai_config_file(app: &AppHandle) -> Option<AiConfigFile> {
    let path = ai_config_file_path(app)?;

    if !path.exists() {
        return None;
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            log::error!(
                "Failed to read AI config file {}: {}",
                path.display(),
                error
            );
            return None;
        }
    };

    match serde_json::from_str::<AiConfigFile>(&contents) {
        Ok(config) => Some(config),
        Err(error) => {
            log::error!(
                "Failed to parse AI config file {}: {}",
                path.display(),
                error
            );
            None
        }
    }
}

fn ai_config_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|directory| directory.join(AI_CONFIG_FILE_NAME))
}

fn clean_optional(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn truncate_error_detail(detail: &str) -> String {
    let mut chars = detail.chars();
    let truncated: String = chars.by_ref().take(MAX_ERROR_DETAIL_CHARS).collect();

    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[tauri::command]
fn toggle_overlay(app: AppHandle) -> Result<(), String> {
    let window = overlay_window(&app)?;
    toggle_window(&window)
}

fn overlay_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| "Overlay window not found".to_string())
}

fn toggle_window(window: &WebviewWindow) -> Result<(), String> {
    let is_visible = window
        .is_visible()
        .map_err(|e| format!("Failed to check visibility: {}", e))?;

    if is_visible {
        window
            .hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    } else {
        show_and_focus_window(window)?;
    }

    Ok(())
}

fn show_and_focus_window(window: &WebviewWindow) -> Result<(), String> {
    window
        .show()
        .map_err(|e| format!("Failed to show window: {}", e))?;
    window
        .set_focus()
        .map_err(|e| format!("Failed to set focus: {}", e))?;

    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = window
            .outer_size()
            .map_err(|e| format!("Failed to read window size: {}", e))?;
        let x = (monitor.size().width as i32 - size.width as i32) / 2 + monitor.position().x;
        let y = (monitor.size().height as i32 - size.height as i32) / 2 + monitor.position().y;

        window
            .set_position(Position::Physical(PhysicalPosition::new(x, y)))
            .map_err(|e| format!("Failed to position window: {}", e))?;
    }

    Ok(())
}

fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_TOGGLE_ID, "Show/Hide Overlay")
        .separator()
        .text(TRAY_QUIT_ID, "Quit App")
        .build()?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Instant")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_TOGGLE_ID => {
                if let Err(error) = toggle_overlay(app.clone()) {
                    log::error!("Failed to toggle overlay from tray: {}", error);
                }
            }
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                if let Err(error) = toggle_overlay(tray.app_handle().clone()) {
                    log::error!("Failed to toggle overlay from tray click: {}", error);
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn global_shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_global_shortcut::ShortcutState;

    tauri_plugin_global_shortcut::Builder::new()
        .with_shortcuts(["ctrl+shift+space"])
        .expect("failed to configure global shortcut")
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(error) = toggle_overlay(app.clone()) {
                    log::error!("Failed to toggle overlay from global shortcut: {}", error);
                }
            }
        })
        .build()
}

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let builder = tauri_plugin_log::Builder::new()
        .level(log::LevelFilter::Info)
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::LogDir {
                file_name: Some("instant".to_string()),
            },
        ));

    #[cfg(debug_assertions)]
    let builder = builder.target(tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::Stdout,
    ));

    builder.build()
}

fn single_instance_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_single_instance::init(|app, args, cwd| {
        log::info!(
            "Blocked duplicate app instance. args={:?}, cwd={}",
            args,
            cwd
        );

        match overlay_window(app) {
            Ok(window) => {
                if let Err(error) = show_and_focus_window(&window) {
                    log::error!("Failed to focus existing overlay window: {}", error);
                }
            }
            Err(error) => log::error!("Failed to resolve overlay window: {}", error),
        }
    })
}

#[cfg(debug_assertions)]
fn load_development_env() {
    let env_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.env");

    if let Err(error) = dotenvy::from_path(&env_path) {
        log::debug!("Development .env not loaded from {:?}: {}", env_path, error);
    }
}

#[cfg(not(debug_assertions))]
fn load_development_env() {}

fn ai_state() -> AiState {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(GEMINI_TIMEOUT_SECS))
        .build()
        .expect("failed to initialize Gemini HTTP client");

    AiState { client }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    load_development_env();

    let builder = tauri::Builder::default()
        .manage(ai_state())
        .plugin(single_instance_plugin())
        .plugin(log_plugin())
        .plugin(tauri_plugin_opener::init());

    #[cfg(target_os = "windows")]
    let builder = builder.plugin(global_shortcut_plugin());

    builder
        .setup(|app| {
            install_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    log::error!("Failed to hide window on close request: {}", error);
                }
            }
            WindowEvent::Focused(true) => {
                if let Err(error) = window.emit("window-focused", ()) {
                    log::error!("Failed to emit window-focused event: {}", error);
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_clipboard_text,
            fetch_ai_response,
            toggle_overlay
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
