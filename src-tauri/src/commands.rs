use crate::batch::runner::{
    new_job_status, plan_items, BatchEvents, FfmpegEncoder, MediaEncoder, SharedBatchState,
};
use crate::batch::{
    BatchItemStatus, BatchJobStatus, BatchPreset, BatchProgressSummary, BatchSettings,
    FfmpegCapabilities, OutputMode,
};
use crate::error_log::{ErrorEntry, SharedErrorLog};
use crate::models::{
    ActionResult, AppSettings, FrontendState, LayoutMode, MediaItem, RenameMode, SortMode,
};
use crate::state::SharedState;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

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

// ---------------------------------------------------------------------------
// Batch optimization / conversion
// ---------------------------------------------------------------------------

const MEDIA_FILTER_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp", "tiff", "tif", "mp4", "mov",
    "m4v", "avi", "mkv", "3gp",
];

/// Forwards runner updates to the window. Channel names are prefixed so they
/// cannot collide with other events.
struct WindowEvents {
    app: AppHandle,
}

impl BatchEvents for WindowEvents {
    fn item(&self, item: &BatchItemStatus) {
        let _ = self.app.emit("batch://item", item);
    }

    fn progress(&self, summary: &BatchProgressSummary) {
        let _ = self.app.emit("batch://progress", summary);
    }

    fn done(&self, job: &BatchJobStatus) {
        let _ = self.app.emit("batch://done", job);
    }
}

#[tauri::command]
pub fn list_queue_items(state: State<'_, SharedState>) -> Result<Vec<MediaItem>, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.list_items())
}

/// Metadata for files picked outside the open album.
#[tauri::command]
pub fn describe_media_paths(paths: Vec<String>) -> Result<Vec<MediaItem>, String> {
    Ok(paths
        .iter()
        .filter(|path| Path::new(path).is_file())
        .map(|path| {
            let mut item = crate::media::build_media_item_from_paths(&[path.clone()]);
            crate::media::enrich_item_metadata(&mut item);
            item
        })
        .collect())
}

#[tauri::command]
pub async fn pick_media_files(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let files = app
        .dialog()
        .file()
        .add_filter("Photos and videos", MEDIA_FILTER_EXTENSIONS)
        .blocking_pick_files();

    Ok(files
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.to_string())
        .collect())
}

#[tauri::command]
pub fn get_ffmpeg_capabilities(
    log: State<'_, SharedErrorLog>,
) -> Result<FfmpegCapabilities, String> {
    match crate::video::FfmpegTools::locate() {
        Ok(tools) => Ok(tools.capabilities()),
        Err(error) => {
            log_rust_error(&log, "get_ffmpeg_capabilities", &error);
            Ok(FfmpegCapabilities::default())
        }
    }
}

#[tauri::command]
pub fn get_batch_presets(state: State<'_, SharedState>) -> Result<Vec<BatchPreset>, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.batch_presets())
}

#[tauri::command]
pub fn save_batch_preset(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    preset: BatchPreset,
) -> Result<Vec<BatchPreset>, String> {
    wrap(
        &log,
        "save_batch_preset",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .save_batch_preset(preset),
    )
}

#[tauri::command]
pub fn delete_batch_preset(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    id: String,
) -> Result<Vec<BatchPreset>, String> {
    wrap(
        &log,
        "delete_batch_preset",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .delete_batch_preset(&id),
    )
}

#[tauri::command]
pub fn get_last_batch_settings(
    state: State<'_, SharedState>,
) -> Result<Option<BatchSettings>, String> {
    Ok(state
        .lock()
        .map_err(|e| e.to_string())?
        .app_settings
        .last_batch_settings
        .clone())
}

/// Starts a batch job. Long encodes run on their own threads: the app-state
/// mutex is never held while ffmpeg works.
#[tauri::command]
pub fn start_batch_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    batch: State<'_, SharedBatchState>,
    log: State<'_, SharedErrorLog>,
    paths: Vec<String>,
    settings: BatchSettings,
) -> Result<BatchJobStatus, String> {
    wrap(&log, "start_batch_job", (|| {
        if paths.is_empty() {
            return Err("Select at least one photo or video.".into());
        }

        // Destructive runs must carry the confirmation the dialog sets. This is
        // checked before a single file is touched.
        if let OutputMode::ReplaceOriginal { confirmed, .. } = &settings.output {
            if !confirmed {
                return Err(
                    "Replacing originals was not confirmed — open the confirmation dialog first."
                        .into(),
                );
            }
        }

        let encoder = Arc::new(FfmpegEncoder::locate()?);

        {
            let runner = batch.lock().map_err(|e| e.to_string())?;
            if let Some(active) = runner.active_job_id() {
                if runner
                    .snapshot(&active)
                    .map(|job| job.running)
                    .unwrap_or(false)
                {
                    return Err("A batch job is already running.".into());
                }
            }
        }

        let plans = plan_items(&paths, &settings);
        let statuses: Vec<BatchItemStatus> =
            plans.iter().map(|planned| planned.status.clone()).collect();
        let output_dir = plans
            .iter()
            .find_map(|planned| planned.plan.as_ref())
            .and_then(|plan| plan.final_output.parent())
            .map(|dir| dir.to_string_lossy().to_string());
        let replaces_originals = matches!(settings.output, OutputMode::ReplaceOriginal { .. });

        let job_id = {
            let mut runner = batch.lock().map_err(|e| e.to_string())?;
            runner.next_job_id()
        };
        let job = Arc::new(Mutex::new(new_job_status(
            job_id.clone(),
            statuses,
            output_dir,
            replaces_originals,
        )));
        let cancel = Arc::new(AtomicBool::new(false));

        {
            let mut runner = batch.lock().map_err(|e| e.to_string())?;
            runner.register(Arc::clone(&job), Arc::clone(&cancel), &job_id);
        }

        if let Ok(mut guard) = state.lock() {
            let _ = guard.remember_batch_settings(&settings);
        }

        let snapshot = job.lock().map_err(|e| e.to_string())?.clone();

        let events = Arc::new(WindowEvents { app: app.clone() });
        let thread_job = Arc::clone(&job);
        std::thread::spawn(move || {
            crate::batch::runner::run_job(thread_job, plans, settings, encoder, events, cancel);
        });

        Ok(snapshot)
    })())
}

#[tauri::command]
pub fn cancel_batch_job(batch: State<'_, SharedBatchState>, job_id: String) -> Result<(), String> {
    batch.lock().map_err(|e| e.to_string())?.cancel(&job_id)
}

#[tauri::command]
pub fn get_batch_job(
    batch: State<'_, SharedBatchState>,
    job_id: String,
) -> Result<BatchJobStatus, String> {
    batch
        .lock()
        .map_err(|e| e.to_string())?
        .snapshot(&job_id)
        .ok_or_else(|| format!("Unknown batch job: {job_id}"))
}

/// Applies a finished job to the open session: registers the undo entry for
/// replaced originals and rebuilds the queue. Safe to call twice.
#[tauri::command]
pub fn finalize_batch_job(
    state: State<'_, SharedState>,
    batch: State<'_, SharedBatchState>,
    log: State<'_, SharedErrorLog>,
    job_id: String,
) -> Result<FrontendState, String> {
    wrap(&log, "finalize_batch_job", (|| {
        let replacements = {
            let runner = batch.lock().map_err(|e| e.to_string())?;
            let handle = runner
                .job(&job_id)
                .ok_or_else(|| format!("Unknown batch job: {job_id}"))?;
            let mut guard = handle.lock().map_err(|e| e.to_string())?;
            if guard.running {
                return Err("The batch job is still running.".into());
            }
            if guard.finalized {
                Vec::new()
            } else {
                guard.finalized = true;
                guard
                    .replacements
                    .iter()
                    .map(|r| {
                        (
                            r.backup_path.clone(),
                            r.original_path.clone(),
                            r.converted_path.clone(),
                        )
                    })
                    .collect()
            }
        };

        {
            let mut runner = batch.lock().map_err(|e| e.to_string())?;
            runner.clear_active(&job_id);
        }

        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.apply_batch_replacements(&replacements)?;
        Ok(guard.to_frontend_state())
    })())
}

/// Duration of one file, for the batch selection list.
#[tauri::command]
pub fn probe_video_duration(path: String) -> Result<Option<f64>, String> {
    let Ok(encoder) = FfmpegEncoder::locate() else {
        return Ok(None);
    };
    Ok(encoder.probe_duration(Path::new(&path)))
}

/// Lists the media in a folder for a batch run, without opening it as an
/// album (no session file is written). `exclude_dirs` keeps a previous run's
/// output from being fed back in.
#[tauri::command]
pub fn scan_folder_media(
    log: State<'_, SharedErrorLog>,
    path: String,
    recursive: bool,
    exclude_dirs: Vec<String>,
) -> Result<Vec<MediaItem>, String> {
    wrap(&log, "scan_folder_media", (|| {
        let root = PathBuf::from(&path);
        if !root.is_dir() {
            return Err(format!("{path} is not a folder."));
        }

        let excluded: Vec<PathBuf> = exclude_dirs
            .iter()
            .filter(|dir| !dir.trim().is_empty())
            .map(PathBuf::from)
            .collect();

        let items = crate::media::scan_folder(&root, recursive)?;
        Ok(items
            .into_iter()
            .filter(|item| {
                !item.paths.iter().any(|file| {
                    excluded
                        .iter()
                        .any(|dir| crate::path_util::is_path_inside_root(dir, Path::new(file)))
                })
            })
            .collect())
    })())
}

/// The job currently registered, if any. Lets the panel re-attach to a run
/// that survived a window reload.
#[tauri::command]
pub fn get_active_batch_job(
    batch: State<'_, SharedBatchState>,
) -> Result<Option<BatchJobStatus>, String> {
    let runner = batch.lock().map_err(|e| e.to_string())?;
    Ok(runner
        .active_job_id()
        .and_then(|job_id| runner.snapshot(&job_id)))
}

/// What the UI needs to talk about updates without hardcoding a repository.
#[derive(serde::Serialize)]
pub struct UpdateContext {
    pub current_version: String,
    /// True when this build was signed and given an update endpoint.
    pub updater_configured: bool,
    /// Releases page of whichever repository published this build.
    pub releases_url: Option<String>,
}

/// Derives the repository from the updater endpoint baked into the build, so a
/// fork points at its own releases with nothing to edit by hand.
fn releases_url_from_endpoint(endpoint: &str) -> Option<String> {
    let marker = "/releases/";
    let index = endpoint.find(marker)?;
    Some(format!("{}/releases", &endpoint[..index]))
}

#[tauri::command]
pub fn get_update_context(app: AppHandle) -> Result<UpdateContext, String> {
    let updater = app
        .config()
        .plugins
        .0
        .get("updater")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let endpoint = updater
        .get("endpoints")
        .and_then(|endpoints| endpoints.as_array())
        .and_then(|endpoints| endpoints.first())
        .and_then(|endpoint| endpoint.as_str())
        .map(str::to_string);

    Ok(UpdateContext {
        current_version: app.package_info().version.to_string(),
        updater_configured: endpoint.is_some(),
        releases_url: endpoint.as_deref().and_then(releases_url_from_endpoint),
    })
}

#[cfg(test)]
mod tests {
    use super::releases_url_from_endpoint;

    #[test]
    fn derives_the_releases_page_from_the_update_endpoint() {
        assert_eq!(
            releases_url_from_endpoint(
                "https://github.com/someone/quick-media-organizer/releases/latest/download/latest.json"
            )
            .as_deref(),
            Some("https://github.com/someone/quick-media-organizer/releases")
        );
        assert_eq!(releases_url_from_endpoint("https://example.com/feed.json"), None);
    }
}
