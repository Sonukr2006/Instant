use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const AUTH_SESSION_FILE_NAME: &str = "auth-session.json";
const KEYCHAIN_SERVICE: &str = "com.sonu.instant.ai";
const KEYCHAIN_ACCOUNT: &str = "instant-auth-token";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthSession {
    pub access_token: String,
}

#[tauri::command]
pub fn get_auth_session(app: AppHandle) -> Result<Option<AuthSession>, String> {
    read_auth_session(&app)
}

#[tauri::command]
pub fn save_auth_session(app: AppHandle, access_token: String) -> Result<AuthSession, String> {
    let access_token = access_token.trim().to_string();

    if access_token.is_empty() {
        return Err("Auth token cannot be empty.".to_string());
    }

    let session = AuthSession { access_token };

    save_keychain_token(&session.access_token)?;
    clear_file_session(&app)?;

    Ok(session)
}

#[tauri::command]
pub fn clear_auth_session(app: AppHandle) -> Result<(), String> {
    clear_keychain_token()?;
    clear_file_session(&app)
}

pub fn saved_auth_token(app: &AppHandle) -> Result<Option<String>, String> {
    Ok(read_auth_session(app)?
        .map(|session| session.access_token)
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty()))
}

fn read_auth_session(app: &AppHandle) -> Result<Option<AuthSession>, String> {
    if let Some(token) = read_keychain_token()? {
        return Ok(Some(AuthSession {
            access_token: token,
        }));
    }

    if let Some(session) = read_file_session(app)? {
        save_keychain_token(&session.access_token)?;
        clear_file_session(app)?;

        return Ok(Some(session));
    }

    Ok(None)
}

fn read_keychain_token() -> Result<Option<String>, String> {
    let entry = keyring_entry()?;

    match entry.get_password() {
        Ok(token) => Ok(Some(token.trim().to_string()).filter(|token| !token.is_empty())),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "Failed to read auth token from secure storage: {error}"
        )),
    }
}

fn save_keychain_token(access_token: &str) -> Result<(), String> {
    let entry = keyring_entry()?;

    entry
        .set_password(access_token)
        .map_err(|error| format!("Failed to save auth token in secure storage: {error}"))
}

fn clear_keychain_token() -> Result<(), String> {
    let entry = keyring_entry()?;

    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Failed to clear auth token from secure storage: {error}"
        )),
    }
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|error| format!("Failed to open secure storage: {error}"))
}

fn read_file_session(app: &AppHandle) -> Result<Option<AuthSession>, String> {
    let path = auth_session_path(app)?;

    if !path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read auth session: {error}"))?;
    let session = serde_json::from_str::<AuthSession>(&contents)
        .map_err(|error| format!("Failed to parse auth session: {error}"))?;

    if session.access_token.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(session))
    }
}

fn clear_file_session(app: &AppHandle) -> Result<(), String> {
    let path = auth_session_path(app)?;

    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to clear legacy auth session: {error}")),
    }
}

fn auth_session_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(AUTH_SESSION_FILE_NAME))
        .map_err(|error| format!("Failed to resolve app config directory: {error}"))
}
