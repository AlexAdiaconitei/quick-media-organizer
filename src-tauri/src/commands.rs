use crate::error_log::{ErrorEntry, SharedErrorLog};
use crate::models::{
    ActionResult, AppSettings, FrontendState, LayoutMode, RenameMode, SortMode,
};
use crate::state::SharedState;
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

fn log_rust_error(log: &SharedErrorLog, command: &str, error: &str) {
    if let Ok(guard) = log.lock() {
        let _ = guard.report(
            "rust",
            "error",
            error,
            Some(json!({ "command": command })),
            None,
        );
    }
}

fn wrap<T>(log: &SharedErrorLog, command: &str, result: Result<T, String>) -> Result<T, String> {
    if let Err(ref error) = result {
        log_rust_error(log, command, error);
    }
    result
}

#[tauri::command]
pub fn report_error(
    log: State<'_, SharedErrorLog>,
    source: String,
    message: String,
    context: Option<serde_json::Value>,
    stack: Option<String>,
) -> Result<ErrorEntry, String> {
    log.lock()
        .map_err(|e| e.to_string())?
        .report(&source, "error", &message, context, stack)
}

#[tauri::command]
pub fn get_error_log(log: State<'_, SharedErrorLog>) -> Result<Vec<ErrorEntry>, String> {
    log.lock().map_err(|e| e.to_string())?.list()
}

#[tauri::command]
pub fn get_error_log_path(log: State<'_, SharedErrorLog>) -> Result<String, String> {
    Ok(log.lock().map_err(|e| e.to_string())?.log_path())
}

#[tauri::command]
pub fn clear_error_log(log: State<'_, SharedErrorLog>) -> Result<(), String> {
    log.lock().map_err(|e| e.to_string())?.clear()
}

#[tauri::command]
pub fn get_app_settings(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
) -> Result<AppSettings, String> {
    wrap(
        &log,
        "get_app_settings",
        state
            .lock()
            .map_err(|e| e.to_string())
            .map(|g| g.app_settings.clone()),
    )
}

#[tauri::command]
pub fn complete_first_run(state: State<'_, SharedState>) -> Result<(), String> {
    state
        .lock()
        .map_err(|e| e.to_string())?
        .complete_first_run()
}

#[tauri::command]
pub fn set_locale(state: State<'_, SharedState>, locale: String) -> Result<(), String> {
    state.lock().map_err(|e| e.to_string())?.set_locale(locale)
}

#[tauri::command]
pub fn set_ui_preferences(
    state: State<'_, SharedState>,
    layout_mode: LayoutMode,
    show_metadata: bool,
    video_with_sound: bool,
) -> Result<AppSettings, String> {
    state
        .lock()
        .map_err(|e| e.to_string())?
        .set_ui_preferences(layout_mode, show_metadata, video_with_sound)
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app
        .dialog()
        .file()
        .blocking_pick_folder();

    Ok(folder.map(|p| p.to_string()))
}

#[tauri::command]
pub fn open_folder(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    path: String,
) -> Result<FrontendState, String> {
    wrap(&log, "open_folder", (|| {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.open_folder(PathBuf::from(path))?;
        let (session_reset, resume_from, subfolder_media_count) = guard.take_transient_open_notices();
        let mut frontend = guard.to_frontend_state();
        frontend.session_reset = session_reset;
        frontend.resume_from = resume_from;
        frontend.subfolder_media_count = subfolder_media_count;
        Ok(frontend)
    })())
}

#[tauri::command]
pub fn get_state(state: State<'_, SharedState>) -> Result<FrontendState, String> {
    Ok(state
        .lock()
        .map_err(|e| e.to_string())?
        .to_frontend_state())
}

#[tauri::command]
pub fn rename_current(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    name: String,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "rename_current",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .rename_current(&name),
    )
}

#[tauri::command]
pub fn trash_current(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "trash_current",
        state.lock().map_err(|e| e.to_string())?.trash_current(),
    )
}

#[tauri::command]
pub fn move_current_to_folder(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    folder: String,
    name: Option<String>,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "move_current_to_folder",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .move_current_to_folder(Some(folder), name),
    )
}

#[tauri::command]
pub fn skip_current(state: State<'_, SharedState>, delta: i32) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.skip(delta)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn dismiss_session_complete(state: State<'_, SharedState>) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.dismiss_session_complete()?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn restart_queue(state: State<'_, SharedState>) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.restart_queue()?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn undo_last(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "undo_last",
        state.lock().map_err(|e| e.to_string())?.undo_last(),
    )
}

#[tauri::command]
pub fn set_armed_folder(
    state: State<'_, SharedState>,
    folder: Option<String>,
) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.set_armed_folder(folder)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn toggle_favorite_folder(
    state: State<'_, SharedState>,
    folder: String,
) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.toggle_favorite(&folder)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn set_options(
    state: State<'_, SharedState>,
    sort_mode: SortMode,
    scan_recursive: bool,
    rename_mode: RenameMode,
) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.set_options(sort_mode, scan_recursive, rename_mode)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn resolve_video_preview(
    state: State<'_, SharedState>,
    path: String,
) -> Result<crate::models::VideoPreviewInfo, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let cache_dir = guard.app_data_dir.join("video-previews");
    Ok(crate::video::resolve_video_preview(
        Path::new(&path),
        &cache_dir,
    ))
}

#[tauri::command]
pub fn diagnose_media_file(path: String) -> Result<crate::models::MediaFileDiagnosis, String> {
    Ok(crate::media::diagnose_media_file(Path::new(&path)))
}

#[tauri::command]
pub fn check_ffmpeg(log: State<'_, SharedErrorLog>) -> Result<bool, String> {
    match crate::video::FfmpegTools::locate() {
        Ok(_) => Ok(true),
        Err(error) => {
            log_rust_error(&log, "check_ffmpeg", &error);
            Ok(false)
        }
    }
}

#[tauri::command]
pub fn trim_current_video(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    trim_start: f64,
    trim_end: f64,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "trim_current_video",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .trim_current_video(trim_start, trim_end),
    )
}
