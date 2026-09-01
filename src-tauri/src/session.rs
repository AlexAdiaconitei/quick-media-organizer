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
    write_atomically(&dir.join(SESSION_FILE), &content)
}

pub fn load_app_settings(app_data_dir: &Path) -> AppSettings {
    let path = app_data_dir.join(SETTINGS_FILE);
    let Ok(content) = fs::read_to_string(&path) else {
        return AppSettings::default();
    };

    match serde_json::from_str(&content) {
        Ok(settings) => settings,
        Err(error) => {
            // Falling back to defaults silently is how the welcome screen came
            // back and favourites disappeared: one unreadable field used to
            // discard the whole file. `AppSettings` now defaults field by
            // field, so reaching here means the JSON itself is broken. Keep it
            // instead of overwriting it, so nothing is lost for good.
            eprintln!("settings.json could not be read ({error}); keeping a copy");
            let _ = fs::rename(&path, path.with_extension("json.corrupt"));
            AppSettings::default()
        }
    }
}

pub fn save_app_settings(app_data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    write_atomically(&app_data_dir.join(SETTINGS_FILE), &content)
}

/// Writes through a temporary file and renames it into place. A plain
/// `fs::write` truncates first, so losing power or being killed mid-write left
/// a half-written file that no longer parsed -- which read to the user as the
/// application forgetting everything at random.
fn write_atomically(path: &Path, content: &str) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, content).map_err(|e| e.to_string())?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LayoutMode;

    /// The exact regression behind "the welcome screen came back": one field
    /// this build cannot read must cost that field, not the whole file.
    #[test]
    fn an_unreadable_field_does_not_reset_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{
                "locale": "es",
                "first_run_completed": true,
                "favorite_folders": ["D:/camera"],
                "layout_mode": "a mode from a newer build",
                "last_batch_settings": { "video": "not an object at all" }
            }"#,
        )
        .unwrap();

        let settings = load_app_settings(dir.path());

        assert!(
            settings.first_run_completed,
            "the welcome screen would return"
        );
        assert_eq!(settings.locale, "es");
        assert_eq!(settings.favorite_folders, vec!["D:/camera".to_string()]);
        // Only the values that could not be read fall back.
        assert_eq!(settings.layout_mode, LayoutMode::default());
        assert!(settings.last_batch_settings.is_none());
        // Nothing was thrown away behind the user's back.
        assert!(!dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn a_file_that_is_not_json_is_kept_instead_of_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS_FILE), "{ truncated mid-writ").unwrap();

        let settings = load_app_settings(dir.path());

        assert!(!settings.first_run_completed);
        assert!(
            dir.path().join("settings.json.corrupt").exists(),
            "the unreadable file must be preserved, not silently replaced"
        );
    }

    #[test]
    fn missing_settings_are_simply_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let settings = load_app_settings(dir.path());
        assert!(!settings.first_run_completed);
        assert!(!dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn saving_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let settings = AppSettings {
            first_run_completed: true,
            ..AppSettings::default()
        };
        save_app_settings(dir.path(), &settings).unwrap();

        assert!(load_app_settings(dir.path()).first_run_completed);
        assert!(!dir.path().join("settings.tmp").exists());
    }

    /// A half-written file used to survive on disk and fail to parse on the
    /// next launch. The rename makes the previous contents last until the new
    /// ones are complete.
    #[test]
    fn a_failed_write_does_not_destroy_the_previous_settings() {
        let dir = tempfile::tempdir().unwrap();
        let settings = AppSettings {
            first_run_completed: true,
            ..AppSettings::default()
        };
        save_app_settings(dir.path(), &settings).unwrap();

        let path = dir.path().join(SETTINGS_FILE);
        let before = fs::read_to_string(&path).unwrap();
        // A directory cannot be renamed over, so the write fails after the
        // temporary file exists: the real file must be untouched.
        fs::create_dir_all(path.with_extension("tmp")).unwrap();
        assert!(save_app_settings(dir.path(), &AppSettings::default()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }
}
