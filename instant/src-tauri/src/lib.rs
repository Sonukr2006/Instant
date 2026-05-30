mod auth;

use auth::{clear_auth_session, get_auth_session, save_auth_session};
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
#[cfg(any(debug_assertions, test))]
const APP_MODE_ENV: &str = "INSTANT_APP_MODE";
const CONTEXT_CAPTURED_EVENT: &str = "context-captured";
const REMOTE_ASK_PATH: &str = "/v1/ai/ask";
#[cfg(target_os = "windows")]
const SELECTED_TEXT_COPY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(650);
#[cfg(target_os = "windows")]
const SELECTED_TEXT_COPY_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);
#[cfg(target_os = "windows")]
const SHORTCUT_RELEASE_SETTLE_DELAY: std::time::Duration = std::time::Duration::from_millis(120);
#[cfg(target_os = "windows")]
const MAX_CLIPBOARD_BACKUP_FORMATS: usize = 64;
#[cfg(target_os = "windows")]
const MAX_CLIPBOARD_BACKUP_BYTES: usize = 32 * 1024 * 1024;
#[cfg(target_os = "windows")]
static WINDOWS_CAPTURE_IN_PROGRESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
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
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<GeminiPromptFeedback>,
}

#[derive(serde::Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct GeminiPromptFeedback {
    #[serde(rename = "blockReason")]
    block_reason: Option<String>,
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
    #[cfg(debug_assertions)]
    app_mode: Option<String>,
    backend_api_url: Option<String>,
    backend_auth_token: Option<String>,
    gemini_api_key: Option<String>,
    gemini_api_version: Option<String>,
    gemini_model: Option<String>,
}

struct BackendConfig {
    api_url: String,
    auth_token: String,
}

enum BackendConfigResolution {
    Configured(BackendConfig),
    MissingApiUrl,
    MissingAuthToken,
    LocalMode,
}

struct AiConfig {
    api_key: String,
    api_version: String,
    model: String,
}

struct AiState {
    client: reqwest::Client,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppMode {
    #[cfg(any(debug_assertions, test))]
    Development,
    Production,
}

impl AppMode {
    fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

#[derive(serde::Serialize)]
struct RemoteAskRequest<'a> {
    prompt_context: &'a str,
}

#[derive(serde::Deserialize)]
struct RemoteAskResponse {
    response_text: String,
}

#[derive(serde::Deserialize)]
struct RemoteErrorResponse {
    error: Option<String>,
}

#[derive(Clone, serde::Serialize)]
struct CapturedContextPayload {
    text: Option<String>,
    error: Option<String>,
    source: &'static str,
}

#[cfg(target_os = "windows")]
struct ClipboardBackup {
    formats: Vec<ClipboardFormatBackup>,
    skipped_formats: usize,
    total_bytes: usize,
}

#[cfg(target_os = "windows")]
struct ClipboardFormatBackup {
    format: u32,
    data: Vec<u8>,
}

#[cfg(target_os = "windows")]
enum SelectedTextCaptureError {
    NoSelectedText,
    Other(String),
}

#[cfg(target_os = "windows")]
impl std::fmt::Display for SelectedTextCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSelectedText => {
                formatter.write_str("No selected text was copied after the shortcut.")
            }
            Self::Other(error) => formatter.write_str(error),
        }
    }
}

#[cfg(target_os = "windows")]
impl From<String> for SelectedTextCaptureError {
    fn from(error: String) -> Self {
        Self::Other(error)
    }
}

#[tauri::command]
fn get_clipboard_text() -> Result<String, String> {
    read_clipboard_text()
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

    let app_mode = resolve_app_mode(&app)?;

    match resolve_backend_config(&app)? {
        BackendConfigResolution::Configured(backend_config) => {
            return fetch_remote_ai_response(&state.client, &backend_config, prompt_context).await;
        }
        BackendConfigResolution::MissingApiUrl => {
            return Err(
                "Login token is saved, but no backend API URL is configured. Set INSTANT_API_BASE_URL or backend_api_url in the app config file."
                    .to_string(),
            );
        }
        BackendConfigResolution::MissingAuthToken => {
            return Err(
                "Backend API URL is configured, but no login token is saved. Connect a login session first."
                    .to_string(),
            );
        }
        BackendConfigResolution::LocalMode if app_mode.is_production() => {
            return Err(
                "Production mode requires a configured Instant backend and login session. Set INSTANT_API_BASE_URL or backend_api_url, then connect a login session."
                    .to_string(),
            );
        }
        BackendConfigResolution::LocalMode => {}
    }

    fetch_local_gemini_response(&state.client, &app, prompt_context).await
}

async fn fetch_remote_ai_response(
    client: &reqwest::Client,
    backend_config: &BackendConfig,
    prompt_context: &str,
) -> Result<String, String> {
    let response = client
        .post(remote_ask_url(&backend_config.api_url))
        .bearer_auth(&backend_config.auth_token)
        .json(&RemoteAskRequest { prompt_context })
        .send()
        .await
        .map_err(|error| format!("Failed to reach Instant backend: {error}"))?;

    let status = response.status();

    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        let message =
            parse_remote_error(&detail).unwrap_or_else(|| truncate_error_detail(detail.trim()));
        let suffix = if message.is_empty() {
            String::new()
        } else {
            format!(" {message}")
        };

        return Err(format!(
            "Instant backend request failed with HTTP {}.{}",
            status.as_u16(),
            suffix
        ));
    }

    response
        .json::<RemoteAskResponse>()
        .await
        .map_err(|error| format!("Failed to parse Instant backend response: {error}"))
        .and_then(|data| {
            let text = data.response_text.trim().to_string();

            if text.is_empty() {
                Err("Instant backend response did not contain generated text.".to_string())
            } else {
                Ok(text)
            }
        })
}

async fn fetch_local_gemini_response(
    client: &reqwest::Client,
    app: &AppHandle,
    prompt_context: &str,
) -> Result<String, String> {
    let ai_config = resolve_ai_config(app)?;
    let model = ai_config.model.trim_start_matches("models/");

    let payload = GeminiRequest {
        contents: vec![GeminiRequestContent {
            parts: vec![GeminiRequestPart {
                text: format!("{AI_SYSTEM_PROMPT}\n\nContext:\n{prompt_context}"),
            }],
        }],
    };

    let response = client
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

    extract_gemini_text(data)
}

fn extract_gemini_text(data: GeminiResponse) -> Result<String, String> {
    let block_reason = data
        .prompt_feedback
        .and_then(|feedback| feedback.block_reason)
        .filter(|reason| !reason.trim().is_empty());

    let Some(candidate) = data
        .candidates
        .and_then(|candidates| candidates.into_iter().next())
    else {
        return Err(gemini_empty_response_error(block_reason, None));
    };

    let finish_reason = candidate
        .finish_reason
        .filter(|reason| !reason.trim().is_empty());
    let text = candidate
        .content
        .and_then(|content| content.parts)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|part| part.text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        Err(gemini_empty_response_error(block_reason, finish_reason))
    } else {
        Ok(text)
    }
}

fn gemini_empty_response_error(
    block_reason: Option<String>,
    finish_reason: Option<String>,
) -> String {
    if let Some(block_reason) = block_reason {
        return format!("Gemini blocked the prompt before generation. Reason: {block_reason}.");
    }

    if let Some(finish_reason) = finish_reason {
        return format!("Gemini finished without generated text. Reason: {finish_reason}.");
    }

    "Gemini response did not contain generated text.".to_string()
}

fn resolve_backend_config(app: &AppHandle) -> Result<BackendConfigResolution, String> {
    let file_config = read_ai_config_file(app);
    let api_url = read_optional_env("INSTANT_API_BASE_URL").or_else(|| {
        file_config
            .as_ref()
            .and_then(|config| clean_optional(&config.backend_api_url))
    });
    let auth_token = if let Some(auth_token) =
        read_optional_env("INSTANT_API_TOKEN").or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| clean_optional(&config.backend_auth_token))
        }) {
        Some(auth_token)
    } else {
        auth::saved_auth_token(app)?
    };

    match (api_url, auth_token) {
        (Some(api_url), Some(auth_token)) => {
            Ok(BackendConfigResolution::Configured(BackendConfig {
                api_url,
                auth_token,
            }))
        }
        (Some(_), None) => Ok(BackendConfigResolution::MissingAuthToken),
        (None, Some(_)) => Ok(BackendConfigResolution::MissingApiUrl),
        (None, None) => Ok(BackendConfigResolution::LocalMode),
    }
}

fn remote_ask_url(api_url: &str) -> String {
    format!("{}{}", api_url.trim_end_matches('/'), REMOTE_ASK_PATH)
}

fn parse_remote_error(detail: &str) -> Option<String> {
    serde_json::from_str::<RemoteErrorResponse>(detail)
        .ok()
        .and_then(|response| response.error)
        .map(|message| truncate_error_detail(message.trim()))
        .filter(|message| !message.is_empty())
}

fn resolve_app_mode(app: &AppHandle) -> Result<AppMode, String> {
    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        return Ok(AppMode::Production);
    }

    #[cfg(debug_assertions)]
    {
        let file_config = read_ai_config_file(app);
        let raw_mode = read_optional_env(APP_MODE_ENV).or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| clean_optional(&config.app_mode))
        });

        raw_mode
            .as_deref()
            .map(parse_app_mode)
            .unwrap_or_else(|| Ok(default_app_mode()))
    }
}

#[cfg(any(debug_assertions, test))]
fn parse_app_mode(value: &str) -> Result<AppMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "development" | "dev" => Ok(AppMode::Development),
        "production" | "prod" => Ok(AppMode::Production),
        _ => Err(format!(
            "{APP_MODE_ENV} must be either 'development' or 'production'."
        )),
    }
}

#[cfg(debug_assertions)]
fn default_app_mode() -> AppMode {
    AppMode::Development
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
    toggle_window_with_clipboard_context(&app, &window)
}

fn overlay_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| "Overlay window not found".to_string())
}

fn toggle_window_with_clipboard_context(
    app: &AppHandle,
    window: &WebviewWindow,
) -> Result<(), String> {
    let is_visible = window
        .is_visible()
        .map_err(|e| format!("Failed to check visibility: {}", e))?;

    if is_visible {
        window
            .hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    } else {
        show_and_focus_window(window)?;
        emit_clipboard_context(app);
    }

    Ok(())
}

fn emit_clipboard_context(app: &AppHandle) {
    emit_context_payload(app, clipboard_context_payload("clipboard"));
}

fn clipboard_context_payload(source: &'static str) -> CapturedContextPayload {
    match read_clipboard_text() {
        Ok(text) => CapturedContextPayload {
            text: Some(text),
            error: None,
            source,
        },
        Err(error) => CapturedContextPayload {
            text: None,
            error: Some(error),
            source,
        },
    }
}

fn read_clipboard_text() -> Result<String, String> {
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

fn emit_context_payload(app: &AppHandle, payload: CapturedContextPayload) {
    if let Err(error) = app.emit(CONTEXT_CAPTURED_EVENT, payload) {
        log::error!("Failed to emit captured context event: {}", error);
    }
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
            if event.state == ShortcutState::Released {
                if let Err(error) = handle_windows_global_shortcut(app) {
                    log::error!("Failed to toggle overlay from global shortcut: {}", error);
                }
            }
        })
        .build()
}

#[cfg(target_os = "windows")]
fn handle_windows_global_shortcut(app: &AppHandle) -> Result<(), String> {
    if WINDOWS_CAPTURE_IN_PROGRESS
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
        )
        .is_err()
    {
        return Ok(());
    }

    let result = handle_windows_global_shortcut_inner(app);
    WINDOWS_CAPTURE_IN_PROGRESS.store(false, std::sync::atomic::Ordering::Release);

    result
}

#[cfg(target_os = "windows")]
fn handle_windows_global_shortcut_inner(app: &AppHandle) -> Result<(), String> {
    let window = overlay_window(app)?;
    let is_visible = window
        .is_visible()
        .map_err(|e| format!("Failed to check visibility: {}", e))?;

    if is_visible {
        window
            .hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
        return Ok(());
    }

    std::thread::sleep(SHORTCUT_RELEASE_SETTLE_DELAY);
    let payload = capture_selected_text_payload();
    show_and_focus_window(&window)?;
    emit_context_payload(app, payload);

    Ok(())
}

#[cfg(target_os = "windows")]
fn capture_selected_text_payload() -> CapturedContextPayload {
    match capture_selected_text_windows() {
        Ok(text) => CapturedContextPayload {
            text: Some(text),
            error: None,
            source: "selected_text",
        },
        Err(SelectedTextCaptureError::NoSelectedText) => CapturedContextPayload {
            text: None,
            error: Some(
                "No selected text was detected. Select text first, then press Ctrl+Shift+Space. For clipboard-only mode, use the tray icon."
                    .to_string(),
            ),
            source: "selected_text",
        },
        Err(error) => {
            let mut payload = clipboard_context_payload("clipboard");
            if payload.error.is_none() {
                payload.error = Some(format!(
                    "Selected text capture failed, so clipboard text was used instead. {error}"
                ));
            } else {
                payload.error = Some(format!(
                    "Selected text capture failed and clipboard text is unavailable. {error}"
                ));
            }
            payload
        }
    }
}

#[cfg(target_os = "windows")]
fn capture_selected_text_windows() -> Result<String, SelectedTextCaptureError> {
    let clipboard_backup = backup_clipboard_windows()?;

    if clipboard_backup.skipped_formats > 0 {
        return Err(SelectedTextCaptureError::Other(format!(
            "Current clipboard has {} unsupported format(s), so selected-text capture was skipped to avoid damaging the user's clipboard.",
            clipboard_backup.skipped_formats
        )));
    }

    let sequence_before = clipboard_sequence_number();

    send_ctrl_c()?;
    let clipboard_changed = wait_for_clipboard_change(sequence_before);

    if !clipboard_changed {
        return Err(SelectedTextCaptureError::NoSelectedText);
    }

    let captured_text = read_clipboard_text();

    if let Err(error) = restore_clipboard_windows(clipboard_backup) {
        log::warn!("Failed to restore previous clipboard state: {}", error);
    }

    captured_text.map_err(SelectedTextCaptureError::Other)
}

#[cfg(target_os = "windows")]
fn backup_clipboard_windows() -> Result<ClipboardBackup, String> {
    let _clipboard = clipboard_win::Clipboard::new_attempts(10).map_err(|error| {
        format!("Unable to access clipboard before selected-text capture: {error}")
    })?;
    let mut formats = Vec::new();
    let mut skipped_formats = 0;
    let mut total_bytes = 0usize;

    for (index, format) in clipboard_win::raw::EnumFormats::new().enumerate() {
        if index >= MAX_CLIPBOARD_BACKUP_FORMATS {
            return Err(format!(
                "Current clipboard has more than {MAX_CLIPBOARD_BACKUP_FORMATS} formats, so selected-text capture was skipped to keep the app responsive."
            ));
        }

        let Some(format_size) = clipboard_win::raw::size(format) else {
            skipped_formats += 1;
            log::warn!(
                "Skipping clipboard format {} because it is not safely readable.",
                format
            );
            continue;
        };
        let format_size = format_size.get();

        if total_bytes.saturating_add(format_size) > MAX_CLIPBOARD_BACKUP_BYTES {
            return Err(format!(
                "Current clipboard is larger than {} MB, so selected-text capture was skipped to avoid high memory use.",
                MAX_CLIPBOARD_BACKUP_BYTES / 1024 / 1024
            ));
        }

        let mut data = Vec::with_capacity(format_size);
        match clipboard_win::raw::get_vec(format, &mut data) {
            Ok(_) => {
                total_bytes = total_bytes.saturating_add(data.len());
                formats.push(ClipboardFormatBackup { format, data });
            }
            Err(error) => {
                skipped_formats += 1;
                log::warn!(
                    "Skipping clipboard format {} because backup failed: {}",
                    format,
                    error
                );
            }
        }
    }

    Ok(ClipboardBackup {
        formats,
        skipped_formats,
        total_bytes,
    })
}

#[cfg(target_os = "windows")]
fn restore_clipboard_windows(backup: ClipboardBackup) -> Result<(), String> {
    let _clipboard = clipboard_win::Clipboard::new_attempts(10)
        .map_err(|error| format!("Unable to access clipboard for restore: {error}"))?;

    clipboard_win::raw::empty()
        .map_err(|error| format!("Unable to clear clipboard before restore: {error}"))?;

    let mut failed_formats = 0usize;
    let mut first_error = None;

    for item in backup.formats {
        if let Err(error) = clipboard_win::raw::set_without_clear(item.format, &item.data) {
            failed_formats += 1;
            let message = format!("format {} restore failed: {error}", item.format);
            log::warn!("{}", message);

            if first_error.is_none() {
                first_error = Some(message);
            }
        }
    }

    if let Some(error) = first_error {
        Err(format!(
            "Clipboard was partially restored; {failed_formats} format(s) failed out of {} backed-up bytes. First failure: {error}",
            backup.total_bytes
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn clipboard_sequence_number() -> u32 {
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

#[cfg(target_os = "windows")]
fn wait_for_clipboard_change(sequence_before: u32) -> bool {
    let started_at = std::time::Instant::now();

    while started_at.elapsed() < SELECTED_TEXT_COPY_TIMEOUT {
        if clipboard_sequence_number() != sequence_before {
            return true;
        }

        std::thread::sleep(SELECTED_TEXT_COPY_POLL_INTERVAL);
    }

    false
}

#[cfg(target_os = "windows")]
fn send_ctrl_c() -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, VK_CONTROL};

    const VK_C: u16 = 0x43;

    let mut inputs = [
        keyboard_input(VK_CONTROL, false),
        keyboard_input(VK_C, false),
        keyboard_input(VK_C, true),
        keyboard_input(VK_CONTROL, true),
    ];
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };

    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err("Windows did not accept the Ctrl+C input sequence.".to_string())
    }
}

#[cfg(target_os = "windows")]
fn keyboard_input(
    virtual_key: u16,
    key_up: bool,
) -> windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
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
            clear_auth_session,
            get_clipboard_text,
            get_auth_session,
            fetch_ai_response,
            save_auth_session,
            toggle_overlay
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_text_parts_are_joined() {
        let response = GeminiResponse {
            candidates: Some(vec![GeminiCandidate {
                content: Some(GeminiResponseContent {
                    parts: Some(vec![
                        GeminiResponsePart {
                            text: Some("alpha".to_string()),
                        },
                        GeminiResponsePart {
                            text: Some("beta".to_string()),
                        },
                    ]),
                }),
                finish_reason: Some("STOP".to_string()),
            }]),
            prompt_feedback: None,
        };

        assert_eq!(
            extract_gemini_text(response).expect("text should parse"),
            "alpha\nbeta"
        );
    }

    #[test]
    fn gemini_block_reason_is_reported() {
        let response = GeminiResponse {
            candidates: None,
            prompt_feedback: Some(GeminiPromptFeedback {
                block_reason: Some("SAFETY".to_string()),
            }),
        };

        assert!(extract_gemini_text(response)
            .expect_err("blocked response should fail")
            .contains("SAFETY"));
    }

    #[test]
    fn app_mode_parser_accepts_supported_values() {
        assert_eq!(
            parse_app_mode("development").expect("valid app mode"),
            AppMode::Development
        );
        assert_eq!(
            parse_app_mode("prod").expect("valid app mode"),
            AppMode::Production
        );
    }

    #[test]
    fn app_mode_parser_rejects_unknown_values() {
        assert!(parse_app_mode("local").is_err());
    }
}
