use crate::models::{AppSettings, SessionData};
use crate::path_util::{APP_FOLDER_NAME, LEGACY_APP_FOLDER_NAME};
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_FILE: &str = "session.json";
const SETTINGS_FILE: &str = "settings.json";

pub fn session_dir_for(folder: &Path) -> PathBuf {
    folder.join(APP_FOLDER_NAME)
}

pub fn load_session(folder: &Path) -> Option<SessionData> {
    let new_path = session_dir_for(folder).join(SESSION_FILE);
    if let Ok(content) = fs::read_to_string(&new_path) {
        if let Ok(session) = serde_json::from_str(&content) {
            return Some(session);
        }
    }

    let legacy_path = folder.join(LEGACY_APP_FOLDER_NAME).join(SESSION_FILE);
    let content = fs::read_to_string(legacy_path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_session(folder: &Path, session: &SessionData) -> Result<(), String> {
    let dir = session_dir_for(folder);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string(session).map_err(|e| e.to_string())?;
    fs::write(dir.join(SESSION_FILE), content).map_err(|e| e.to_string())
}

pub fn load_app_settings(app_data_dir: &Path) -> AppSettings {
    let path = app_data_dir.join(SETTINGS_FILE);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save_app_settings(app_data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(app_data_dir.join(SETTINGS_FILE), content).map_err(|e| e.to_string())
}
